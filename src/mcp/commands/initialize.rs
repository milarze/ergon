use serde::{Deserialize, Serialize};

use super::{default_jsonrpc, CommandId, CommandMethod};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Initialize {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: CommandId,
    pub method: CommandMethod,
    pub params: InitializeParams,
}

impl Initialize {
    pub fn new(params: InitializeParams) -> Self {
        Initialize {
            jsonrpc: default_jsonrpc(),
            id: CommandId::default(),
            method: CommandMethod::Initialize,
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
    #[serde(rename = "capabilities")]
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(rename = "roots", skip_serializing_if = "Option::is_none")]
    pub roots: Option<Roots>,
    #[serde(rename = "sampling", skip_serializing_if = "Option::is_none")]
    pub sampling: Option<Sampling>,
    #[serde(rename = "elicitation", skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<Elicitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roots {
    #[serde(rename = "listChanged", default)]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sampling {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Elicitation {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_structure() {
        let init = Initialize {
            jsonrpc: default_jsonrpc(),
            id: CommandId::default(),
            method: CommandMethod::Initialize,
            params: InitializeParams {
                protocol_version: "1.0".to_string(),
                client_info: ClientInfo {
                    name: "Ergon".to_string(),
                    title: "Ergon Client".to_string(),
                    version: "0.1.0".to_string(),
                },
                capabilities: Capabilities {
                    roots: Some(Roots {
                        list_changed: false,
                    }),
                    sampling: Some(Sampling {}),
                    elicitation: Some(Elicitation {}),
                },
            },
        };
        assert_eq!(init.jsonrpc, "2.0");
        if let CommandId::String(id) = init.id {
            assert_eq!(id.len(), 10);
        } else {
            panic!("Expected CommandId to be String");
        }
        assert_eq!(init.method, CommandMethod::Initialize);
        assert_eq!(init.params.protocol_version, "1.0");
        assert_eq!(init.params.client_info.name, "Ergon");
        assert_eq!(init.params.client_info.title, "Ergon Client");
        assert_eq!(init.params.client_info.version, "0.1.0");
        assert!(matches!(
            init.params.capabilities.roots,
            Some(Roots {
                list_changed: false
            })
        ));
        assert!(matches!(
            init.params.capabilities.sampling,
            Some(Sampling {})
        ));
        assert!(matches!(
            init.params.capabilities.elicitation,
            Some(Elicitation {})
        ));
    }

    #[test]
    fn test_capabilities_serialization() {
        let capabilities = Capabilities {
            roots: Some(Roots {
                list_changed: false,
            }),
            sampling: Some(Sampling {}),
            elicitation: Some(Elicitation {}),
        };
        let json_str =
            serde_json::to_string(&capabilities).expect("Failed to serialize Capabilities");
        assert_eq!(
            json_str,
            r#"{"roots":{"listChanged":false},"sampling":{},"elicitation":{}}"#
        );
    }

    #[test]
    fn test_capabilities_serialization_without_sampling() {
        let capabilities = Capabilities {
            roots: Some(Roots {
                list_changed: false,
            }),
            sampling: None,
            elicitation: Some(Elicitation {}),
        };
        let json_str =
            serde_json::to_string(&capabilities).expect("Failed to serialize Capabilities");
        assert_eq!(
            json_str,
            r#"{"roots":{"listChanged":false},"elicitation":{}}"#
        );
    }

    #[test]
    fn test_capabilities_serialization_without_elicitation() {
        let capabilities = Capabilities {
            roots: Some(Roots {
                list_changed: false,
            }),
            sampling: Some(Sampling {}),
            elicitation: None,
        };
        let json_str =
            serde_json::to_string(&capabilities).expect("Failed to serialize Capabilities");
        assert_eq!(json_str, r#"{"roots":{"listChanged":false},"sampling":{}}"#);
    }

    #[test]
    fn test_capabilities_serialization_without_roots() {
        let capabilities = Capabilities {
            roots: None,
            sampling: Some(Sampling {}),
            elicitation: Some(Elicitation {}),
        };
        let json_str =
            serde_json::to_string(&capabilities).expect("Failed to serialize Capabilities");
        assert_eq!(json_str, r#"{"sampling":{},"elicitation":{}}"#);
    }

    #[test]
    fn test_params_serialization() {
        let params = InitializeParams {
            protocol_version: "1.0".to_string(),
            client_info: ClientInfo {
                name: "Ergon".to_string(),
                title: "Ergon Client".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: Capabilities {
                roots: Some(Roots {
                    list_changed: false,
                }),
                sampling: Some(Sampling {}),
                elicitation: Some(Elicitation {}),
            },
        };
        let json_str =
            serde_json::to_string(&params).expect("Failed to serialize InitializeParams");
        assert_eq!(
            json_str,
            r#"{"protocolVersion":"1.0","clientInfo":{"name":"Ergon","title":"Ergon Client","version":"0.1.0"},"capabilities":{"roots":{"listChanged":false},"sampling":{},"elicitation":{}}}"#
        );
    }

    #[test]
    fn test_roots_serialization() {
        let roots = Roots {
            list_changed: false,
        };
        let json_str = serde_json::to_string(&roots).expect("Failed to serialize Roots");
        assert_eq!(json_str, r#"{"listChanged":false}"#);
    }

    #[test]
    fn test_sampling_serialization() {
        let sampling = Sampling {};
        let json_str = serde_json::to_string(&sampling).expect("Failed to serialize Sampling");
        assert_eq!(json_str, "{}");
    }

    #[test]
    fn test_elicitation_serialization() {
        let elicitation = Elicitation {};
        let json_str =
            serde_json::to_string(&elicitation).expect("Failed to serialize Elicitation");
        assert_eq!(json_str, "{}");
    }

    #[test]
    fn test_client_info_serialization() {
        let client_info = ClientInfo {
            name: "Ergon".to_string(),
            title: "Ergon Client".to_string(),
            version: "0.1.0".to_string(),
        };
        let json_str = serde_json::to_string(&client_info).expect("Failed to serialize ClientInfo");
        assert_eq!(
            json_str,
            r#"{"name":"Ergon","title":"Ergon Client","version":"0.1.0"}"#
        );
    }

    #[test]
    fn test_initialize_new() {
        let params = InitializeParams {
            protocol_version: "1.0".to_string(),
            client_info: ClientInfo {
                name: "Ergon".to_string(),
                title: "Ergon Client".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: Capabilities {
                roots: Some(Roots {
                    list_changed: false,
                }),
                sampling: Some(Sampling {}),
                elicitation: Some(Elicitation {}),
            },
        };
        let init = Initialize::new(params);
        assert_eq!(init.jsonrpc, "2.0");
        assert!(matches!(init.id, CommandId::String(_)));
        assert_eq!(init.method, CommandMethod::Initialize);
    }

    #[test]
    fn test_initialize_new_without_capabilities() {
        let params = InitializeParams {
            protocol_version: "1.0".to_string(),
            client_info: ClientInfo {
                name: "Ergon".to_string(),
                title: "Ergon Client".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: Capabilities {
                roots: None,
                sampling: None,
                elicitation: None,
            },
        };
        let init = Initialize::new(params);
        assert_eq!(init.jsonrpc, "2.0");
        assert!(matches!(init.id, CommandId::String(_)));
        assert_eq!(init.method, CommandMethod::Initialize);
        assert!(init.params.capabilities.roots.is_none());
        assert!(init.params.capabilities.sampling.is_none());
        assert!(init.params.capabilities.elicitation.is_none());
    }
}
