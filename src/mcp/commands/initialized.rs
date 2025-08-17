use super::{default_jsonrpc, CommandMethod};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Initialized {
    #[serde(default = "default_jsonrpc")]
    jsonrpc: String,
    #[serde(default = "method")]
    method: CommandMethod,
}

fn method() -> CommandMethod {
    CommandMethod::Initialized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialized_serialization() {
        let initialized = Initialized {
            jsonrpc: default_jsonrpc(),
            method: CommandMethod::Initialized,
        };
        let json_str =
            serde_json::to_string(&initialized).expect("Failed to serialize Initialized");
        assert_eq!(
            json_str,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        );
    }
}
