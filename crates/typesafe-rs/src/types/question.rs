use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::types::entry::Entry;

/// Maximum number of Choice options accepted by client-side validation.
pub const MAX_CHOICE_OPTIONS: usize = 255;

/// Ordered map of named questions, keyed as they should appear in answers.
pub type Questions = IndexMap<String, Question>;

/// A typed System One question.
///
/// Build with [`Question::noul`], [`Question::choice`], and [`Question::score`].
/// Choice needs at least two [`option`](Self::option)s; Score needs at least two
/// [`level`](Self::level)s. Validation runs before the request is sent.
///
/// # Examples
///
/// ```
/// use typesafe_rs::Question;
///
/// let urgent = Question::noul("Does this convey urgency?")
///     .when_true("Explicitly time-sensitive")
///     .when_false("No time pressure");
/// let team = Question::choice("Which team?")
///     .option("billing", "Payments")
///     .option("technical", "Bugs");
/// let mood = Question::score("How frustrated?")
///     .level("Calm")
///     .level("Frustrated")
///     .level("Very angry");
/// # let _ = (urgent, team, mood);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes/no question; the answer is a probability in `[0, 1]`.
    Noul {
        /// The question to evaluate.
        instructions: Entry,
        /// Optional descriptions of the yes and no outcomes.
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// Select one named option from a set.
    Choice {
        /// What the model should decide.
        instructions: Entry,
        /// Option labels mapped to descriptions (`null` if undescribed).
        criteria: IndexMap<String, Entry>,
    },
    /// Rate the state against an ordered rubric.
    Score {
        /// What the model should rate.
        instructions: Entry,
        /// Level descriptions; index is the integer score.
        criteria: Vec<Entry>,
    },
}

/// Optional yes/no descriptions for a Noul question.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoulCriteria {
    /// Description of a yes outcome (wire name `true`).
    #[serde(rename = "true", skip_serializing_if = "Option::is_none")]
    pub when_true: Option<Entry>,
    /// Description of a no outcome (wire name `false`).
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")]
    pub when_false: Option<Entry>,
}

impl Question {
    /// Build a Noul question.
    #[must_use]
    pub fn noul(instructions: impl Into<Entry>) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: None,
        }
    }

    /// Build a Choice question with no options yet.
    #[must_use]
    pub fn choice(instructions: impl Into<Entry>) -> Self {
        Self::Choice {
            instructions: instructions.into(),
            criteria: IndexMap::new(),
        }
    }

    /// Build a Score question with no levels yet.
    #[must_use]
    pub fn score(instructions: impl Into<Entry>) -> Self {
        Self::Score {
            instructions: instructions.into(),
            criteria: Vec::new(),
        }
    }

    /// Describe the yes outcome of a Noul question.
    ///
    /// No-op if this is not a Noul question.
    #[must_use]
    pub fn when_true(self, description: impl Into<Entry>) -> Self {
        match self {
            Self::Noul {
                instructions,
                criteria,
            } => {
                let mut criteria = criteria.unwrap_or_default();
                criteria.when_true = Some(description.into());
                Self::Noul {
                    instructions,
                    criteria: Some(criteria),
                }
            }
            other => other,
        }
    }

    /// Describe the no outcome of a Noul question.
    ///
    /// No-op if this is not a Noul question.
    #[must_use]
    pub fn when_false(self, description: impl Into<Entry>) -> Self {
        match self {
            Self::Noul {
                instructions,
                criteria,
            } => {
                let mut criteria = criteria.unwrap_or_default();
                criteria.when_false = Some(description.into());
                Self::Noul {
                    instructions,
                    criteria: Some(criteria),
                }
            }
            other => other,
        }
    }

    /// Add a named option to a Choice question.
    ///
    /// No-op if this is not a Choice question.
    #[must_use]
    pub fn option(self, key: impl Into<String>, description: impl Into<Entry>) -> Self {
        match self {
            Self::Choice {
                instructions,
                mut criteria,
            } => {
                criteria.insert(key.into(), description.into());
                Self::Choice {
                    instructions,
                    criteria,
                }
            }
            other => other,
        }
    }

    /// Append a rubric level to a Score question.
    ///
    /// No-op if this is not a Score question.
    #[must_use]
    pub fn level(self, description: impl Into<Entry>) -> Self {
        match self {
            Self::Score {
                instructions,
                mut criteria,
            } => {
                criteria.push(description.into());
                Self::Score {
                    instructions,
                    criteria,
                }
            }
            other => other,
        }
    }
}

/// Validate a question map before sending it on the wire.
pub fn validate_questions(questions: &Questions) -> Result<(), Error> {
    if questions.is_empty() {
        return Err(Error::InvalidRequest(
            "At least one question is required.".to_owned(),
        ));
    }
    for (name, question) in questions {
        if name.is_empty() {
            return Err(Error::InvalidRequest(
                "question keys must be non-empty".to_owned(),
            ));
        }
        match question {
            Question::Choice { criteria, .. } => {
                let n = criteria.len();
                if n < 2 {
                    return Err(Error::InvalidRequest(format!(
                        "Choice question \"{name}\" has {n} options; at least 2 are required"
                    )));
                }
                if n > MAX_CHOICE_OPTIONS {
                    return Err(Error::InvalidRequest(format!(
                        "Choice question \"{name}\" has {n} options; at most {MAX_CHOICE_OPTIONS} are allowed"
                    )));
                }
            }
            Question::Score { criteria, .. } => {
                let n = criteria.len();
                if n < 2 {
                    return Err(Error::InvalidRequest(format!(
                        "Score question \"{name}\" has {n} criteria; at least two scores are required."
                    )));
                }
            }
            Question::Noul { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn noul_serializes_with_criteria_keys() {
        let q = Question::noul("Does this convey urgency?")
            .when_true("Explicitly time-sensitive")
            .when_false("No time pressure");
        let value = serde_json::to_value(&q).unwrap();
        assert_eq!(
            value,
            json!({
                "type": "noul",
                "instructions": "Does this convey urgency?",
                "criteria": {
                    "true": "Explicitly time-sensitive",
                    "false": "No time pressure"
                }
            })
        );
    }

    #[test]
    fn noul_omits_empty_criteria() {
        let q = Question::noul("yes?");
        let value = serde_json::to_value(&q).unwrap();
        assert_eq!(value.get("criteria"), None);
    }

    #[test]
    fn choice_preserves_insertion_order() {
        let q = Question::choice("Which team?")
            .option("billing", "Payment issues")
            .option("technical", "Bugs");
        let value = serde_json::to_value(&q).unwrap();
        let keys: Vec<_> = value["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["billing", "technical"]);
    }

    #[test]
    fn score_serializes_levels_as_array() {
        let q = Question::score("How frustrated?")
            .level("Calm")
            .level("Frustrated")
            .level("Very angry");
        let value = serde_json::to_value(&q).unwrap();
        assert_eq!(
            value["criteria"],
            json!(["Calm", "Frustrated", "Very angry"])
        );
    }

    #[test]
    fn rejects_empty_map() {
        let q = Questions::new();
        let err = validate_questions(&q).unwrap_err();
        assert!(matches!(err, Error::InvalidRequest(_)));
    }

    #[test]
    fn rejects_empty_key() {
        let mut q = Questions::new();
        q.insert(String::new(), Question::noul("x"));
        let err = validate_questions(&q).unwrap_err();
        assert!(format!("{err}").contains("non-empty"));
    }

    #[test]
    fn rejects_choice_with_one_option() {
        let mut q = Questions::new();
        q.insert(
            "dept".into(),
            Question::choice("Which?").option("only", "one"),
        );
        let err = validate_questions(&q).unwrap_err();
        assert!(format!("{err}").contains("at least 2"));
    }

    #[test]
    fn rejects_choice_over_max_options() {
        let mut criteria = IndexMap::new();
        for i in 0..=MAX_CHOICE_OPTIONS {
            criteria.insert(format!("k{i}"), Entry::from("d"));
        }
        let mut q = Questions::new();
        q.insert(
            "dept".into(),
            Question::Choice {
                instructions: Entry::from("Which?"),
                criteria,
            },
        );
        let err = validate_questions(&q).unwrap_err();
        assert!(format!("{err}").contains("at most"));
    }

    #[test]
    fn rejects_score_with_one_level() {
        let mut q = Questions::new();
        q.insert("s".into(), Question::score("rate").level("low"));
        let err = validate_questions(&q).unwrap_err();
        assert!(format!("{err}").contains("at least two"));
    }

    #[test]
    fn accepts_valid_choice_and_score() {
        let mut q = Questions::new();
        q.insert(
            "dept".into(),
            Question::choice("Which?").option("a", "A").option("b", "B"),
        );
        q.insert(
            "mood".into(),
            Question::score("rate").level("low").level("high"),
        );
        validate_questions(&q).unwrap();
    }
}
