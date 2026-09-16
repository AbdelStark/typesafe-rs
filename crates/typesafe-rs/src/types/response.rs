use http::{HeaderMap, StatusCode};
use indexmap::IndexMap;
use serde::Deserialize;

/// Token usage reported by the API.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Usage {
    /// Prompt / input tokens.
    pub input_tokens: u64,
    /// Completion / output tokens.
    pub output_tokens: u64,
}

/// Metadata captured from the HTTP response (not part of the JSON body).
#[derive(Clone, Debug, Default)]
pub struct ResponseMeta {
    /// Value of `x-typesafe-request-id` when present.
    pub request_id: Option<String>,
    /// HTTP status of the final attempt.
    pub status: Option<StatusCode>,
    /// Response headers of the final attempt.
    pub headers: HeaderMap,
    /// Total HTTP attempts including the original request.
    pub attempts: u32,
}

/// A typed answer, or [`Answer::Unknown`] for forward-compatible types.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    /// Yes/no probability.
    Noul {
        /// Probability of yes, in `[0, 1]`.
        noul: f64,
    },
    /// Selected label plus the full distribution.
    Choice {
        /// Highest-probability option.
        choice: String,
        /// Probability of each option.
        probabilities: IndexMap<String, f64>,
        /// Model confidence in the selected label.
        confidence: f64,
    },
    /// Weighted score across ordered levels.
    Score {
        /// Expected score; may fall between integer levels.
        score: f64,
        /// Level index (as a string) mapped to its description.
        legend: IndexMap<String, serde_json::Value>,
        /// Optional per-level probabilities.
        #[serde(default)]
        probabilities: Option<IndexMap<String, f64>>,
        /// Model confidence in the score.
        confidence: f64,
    },
    /// An answer `type` this SDK version does not model.
    #[serde(other)]
    Unknown,
}

/// Borrowed view of a Noul answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoulView {
    /// Probability of yes, in `[0, 1]`.
    pub noul: f64,
}

/// Borrowed view of a Choice answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChoiceView<'a> {
    /// Selected option.
    pub choice: &'a str,
    /// Probability of each option.
    pub probabilities: &'a IndexMap<String, f64>,
    /// Model confidence.
    pub confidence: f64,
}

/// Borrowed view of a Score answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreView<'a> {
    /// Expected score.
    pub score: f64,
    /// Level descriptions keyed by index.
    pub legend: &'a IndexMap<String, serde_json::Value>,
    /// Per-level probabilities, when the API included them.
    pub probabilities: Option<&'a IndexMap<String, f64>>,
    /// Model confidence.
    pub confidence: f64,
}

/// Parsed `POST /v1/systemone` response.
#[derive(Clone, Debug, Deserialize)]
pub struct SystemOneResponse {
    /// Model that produced the answers.
    pub model: String,
    /// Answers keyed as in the request.
    pub answers: IndexMap<String, Answer>,
    /// Token usage, when present.
    #[serde(default)]
    pub usage: Option<Usage>,
    /// HTTP metadata filled in by the client after deserialize.
    #[serde(skip)]
    pub meta: ResponseMeta,
}

impl SystemOneResponse {
    /// Iterate Noul answers.
    pub fn nouls(&self) -> impl Iterator<Item = (&str, NoulView)> + '_ {
        self.answers
            .iter()
            .filter_map(|(key, answer)| match answer {
                Answer::Noul { noul } => Some((key.as_str(), NoulView { noul: *noul })),
                _ => None,
            })
    }

    /// Iterate Choice answers.
    pub fn choices(&self) -> impl Iterator<Item = (&str, ChoiceView<'_>)> + '_ {
        self.answers
            .iter()
            .filter_map(|(key, answer)| match answer {
                Answer::Choice {
                    choice,
                    probabilities,
                    confidence,
                } => Some((
                    key.as_str(),
                    ChoiceView {
                        choice,
                        probabilities,
                        confidence: *confidence,
                    },
                )),
                _ => None,
            })
    }

    /// Iterate Score answers.
    pub fn scores(&self) -> impl Iterator<Item = (&str, ScoreView<'_>)> + '_ {
        self.answers
            .iter()
            .filter_map(|(key, answer)| match answer {
                Answer::Score {
                    score,
                    legend,
                    probabilities,
                    confidence,
                } => Some((
                    key.as_str(),
                    ScoreView {
                        score: *score,
                        legend,
                        probabilities: probabilities.as_ref(),
                        confidence: *confidence,
                    },
                )),
                _ => None,
            })
    }

    /// Noul probability for `key`, if that answer is a Noul.
    #[must_use]
    pub fn noul(&self, key: &str) -> Option<f64> {
        match self.answers.get(key) {
            Some(Answer::Noul { noul }) => Some(*noul),
            _ => None,
        }
    }

    /// Choice view for `key`, if that answer is a Choice.
    #[must_use]
    pub fn choice(&self, key: &str) -> Option<ChoiceView<'_>> {
        match self.answers.get(key) {
            Some(Answer::Choice {
                choice,
                probabilities,
                confidence,
            }) => Some(ChoiceView {
                choice,
                probabilities,
                confidence: *confidence,
            }),
            _ => None,
        }
    }

    /// Score view for `key`, if that answer is a Score.
    #[must_use]
    pub fn score(&self, key: &str) -> Option<ScoreView<'_>> {
        match self.answers.get(key) {
            Some(Answer::Score {
                score,
                legend,
                probabilities,
                confidence,
            }) => Some(ScoreView {
                score: *score,
                legend,
                probabilities: probabilities.as_ref(),
                confidence: *confidence,
            }),
            _ => None,
        }
    }

    /// Borrow the answer for `key`.
    #[must_use]
    pub fn answer(&self, key: &str) -> Option<&Answer> {
        self.answers.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_noul_choice_score() {
        let body = json!({
            "model": "jev-latest",
            "answers": {
                "urgent": { "type": "noul", "noul": 0.92 },
                "dept": {
                    "type": "choice",
                    "choice": "technical",
                    "probabilities": { "billing": 0.08, "technical": 0.85, "sales": 0.07 },
                    "confidence": 0.82
                },
                "frustration": {
                    "type": "score",
                    "score": 1.6,
                    "legend": { "0": "Calm", "1": "Frustrated", "2": "Very angry" },
                    "probabilities": { "0": 0.05, "1": 0.3, "2": 0.65 },
                    "confidence": 0.78
                }
            },
            "usage": { "input_tokens": 312, "output_tokens": 48 }
        });
        let resp: SystemOneResponse = serde_json::from_value(body).unwrap();
        assert_eq!(resp.noul("urgent"), Some(0.92));
        assert_eq!(resp.choice("dept").unwrap().choice, "technical");
        assert_eq!(resp.score("frustration").unwrap().score, 1.6);
        assert_eq!(resp.usage.as_ref().unwrap().input_tokens, 312);
        assert_eq!(resp.nouls().count(), 1);
        assert_eq!(resp.choices().count(), 1);
        assert_eq!(resp.scores().count(), 1);
    }

    #[test]
    fn unknown_answer_type_does_not_fail() {
        let body = json!({
            "model": "jev-latest",
            "answers": {
                "future": { "type": "spectrum", "value": 1 },
                "urgent": { "type": "noul", "noul": 0.5 }
            }
        });
        let resp: SystemOneResponse = serde_json::from_value(body).unwrap();
        assert!(matches!(resp.answer("future"), Some(Answer::Unknown)));
        assert_eq!(resp.noul("urgent"), Some(0.5));
    }

    #[test]
    fn extra_fields_on_known_answers_are_ignored() {
        let body = json!({
            "model": "jev-latest",
            "answers": {
                "urgent": { "type": "noul", "noul": 0.1, "extra": true }
            },
            "bonus": 1
        });
        let resp: SystemOneResponse = serde_json::from_value(body).unwrap();
        assert_eq!(resp.noul("urgent"), Some(0.1));
    }

    #[test]
    fn score_without_probabilities_is_ok() {
        let body = json!({
            "model": "jev-latest",
            "answers": {
                "s": {
                    "type": "score",
                    "score": 0.0,
                    "legend": { "0": "a", "1": "b" },
                    "confidence": 0.5
                }
            }
        });
        let resp: SystemOneResponse = serde_json::from_value(body).unwrap();
        assert!(resp.score("s").unwrap().probabilities.is_none());
    }
}
