use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Text, structured JSON, or `null` for state, instructions, and criteria.
///
/// Matches TypeSafe's `string | object | array | null` entry type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    /// A text instruction or description.
    Text(Cow<'static, str>),
    /// JSON `null`.
    #[default]
    Null,
    /// A JSON object, array, number, or boolean.
    Json(serde_json::Value),
}

impl From<&'static str> for Entry {
    fn from(value: &'static str) -> Self {
        Self::Text(Cow::Borrowed(value))
    }
}

impl From<String> for Entry {
    fn from(value: String) -> Self {
        Self::Text(Cow::Owned(value))
    }
}

impl From<&String> for Entry {
    fn from(value: &String) -> Self {
        Self::Text(Cow::Owned(value.clone()))
    }
}

impl From<serde_json::Value> for Entry {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::String(s) => Self::Text(Cow::Owned(s)),
            other => Self::Json(other),
        }
    }
}

impl From<Option<String>> for Entry {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(s) => Self::from(s),
            None => Self::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_roundtrip() {
        let entry = Entry::from("hello");
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value, json!("hello"));
        let back: Entry = serde_json::from_value(value).unwrap();
        assert_eq!(back, Entry::from("hello"));
    }

    #[test]
    fn null_roundtrip() {
        let value = serde_json::to_value(Entry::Null).unwrap();
        assert_eq!(value, json!(null));
        let back: Entry = serde_json::from_value(json!(null)).unwrap();
        assert_eq!(back, Entry::Null);
    }

    #[test]
    fn object_roundtrip() {
        let entry = Entry::from(json!({"question": "urgent?"}));
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value, json!({"question": "urgent?"}));
    }

    #[test]
    fn array_roundtrip() {
        let entry = Entry::from(json!(["a", "b"]));
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value, json!(["a", "b"]));
    }

    #[test]
    fn from_value_null_is_null_variant() {
        assert_eq!(Entry::from(serde_json::Value::Null), Entry::Null);
    }
}
