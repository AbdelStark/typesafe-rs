use serde::{Deserialize, Serialize};

use crate::types::question::Questions;

/// Request body for `POST /v1/systemone`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemOneRequest {
    /// Content to evaluate: a string, object, or array.
    pub state: serde_json::Value,
    /// Model name. Empty values inherit the client's default at send time.
    pub model: String,
    /// Named questions; answers come back under the same keys.
    pub questions: Questions,
}

impl SystemOneRequest {
    /// Build a request that inherits the client's default model.
    #[must_use]
    pub fn new(state: serde_json::Value, questions: Questions) -> Self {
        Self {
            state,
            model: String::new(),
            questions,
        }
    }

    /// Override the model for this request.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::question::Question;
    use serde_json::json;

    #[test]
    fn serializes_state_model_questions() {
        let mut questions = Questions::new();
        questions.insert("urgent".into(), Question::noul("urgent?"));
        let req = SystemOneRequest::new(json!("hello"), questions).with_model("jev-latest");
        let value = serde_json::to_value(&req).unwrap();
        assert_eq!(value["state"], json!("hello"));
        assert_eq!(value["model"], "jev-latest");
        assert_eq!(value["questions"]["urgent"]["type"], "noul");
    }
}
