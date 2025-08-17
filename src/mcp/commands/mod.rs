pub mod initialize;
pub mod initialized;

use serde::{Deserialize, Serialize};

use rand::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandId {
    String(String),
    Integer(i64),
}

impl Default for CommandId {
    fn default() -> Self {
        CommandId::String(
            rand::rng()
                .sample_iter(rand::distr::Alphanumeric)
                .take(10)
                .map(char::from)
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandMethod {
    #[serde(rename = "initialize")]
    Initialize,
    #[serde(rename = "notifications/initialized")]
    Initialized,
}

fn default_jsonrpc() -> String {
    "2.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_id_default() {
        let id = CommandId::default();
        if let CommandId::String(s) = id {
            assert_eq!(s.len(), 10);
        } else {
            panic!("Expected CommandId to be String");
        }
    }

    #[test]
    fn test_command_id_serialization() {
        let id = CommandId::String("test_id".to_string());
        let json_str = serde_json::to_string(&id).expect("Failed to serialize CommandId");
        assert_eq!(json_str, r#""test_id""#);
    }

    #[test]
    fn test_command_method_initialize() {
        let init_json_str = serde_json::to_string(&CommandMethod::Initialize)
            .expect("Failed to serialize CommandMethod::Initialize");
        assert_eq!(init_json_str, r#""initialize""#);
    }

    #[test]
    fn test_default_jsonrpc() {
        let jsonrpc = default_jsonrpc();
        assert_eq!(jsonrpc, "2.0");
    }
}
