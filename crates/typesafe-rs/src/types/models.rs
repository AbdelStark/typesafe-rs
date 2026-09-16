use serde::Deserialize;

/// Metadata for an available model.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModelCard {
    /// Model identifier, e.g. `jev-latest`.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Release date as returned by the API.
    pub release_date: String,
}

/// Wire body for `GET /v1/models`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ListModelsResponse {
    /// Available models.
    pub models: Vec<ModelCard>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_models_array() {
        let body = json!({
            "models": [
                {
                    "name": "jev-latest",
                    "description": "Flagship",
                    "release_date": "2026-01-01"
                }
            ]
        });
        let parsed: ListModelsResponse = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.models[0].name, "jev-latest");
    }
}
