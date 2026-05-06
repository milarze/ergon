//! Convert Ergon's [`McpConfig`] entries into ACP [`McpServer`] entries to
//! pass through during `session/new` and `session/load`.
//!
//! Stdio MCP servers are always supported by ACP agents. HTTP/SSE servers
//! are gated on `agent_capabilities.mcp_capabilities.{http,sse}`. Entries
//! with empty fields, or HTTP servers with OAuth2 auth (whose tokens we
//! cannot safely surface to the agent process), are silently dropped.
//!
//! See protocol docs: <https://agentclientprotocol.com/protocol/session-setup#mcp-servers>

use std::path::PathBuf;

use agent_client_protocol::schema::{
    HttpHeader, McpCapabilities, McpServer, McpServerHttp, McpServerStdio,
};

use crate::config::{McpAuthConfig, McpConfig};

/// Map a slice of Ergon MCP configs into ACP `McpServer` entries.
///
/// `caps` is read from the agent's initialize response and used to gate
/// HTTP/SSE servers. Stdio entries are always included if the command path
/// is non-empty.
pub fn mcp_servers_from_configs(
    configs: &[McpConfig],
    caps: &McpCapabilities,
) -> Vec<McpServer> {
    let mut out = Vec::with_capacity(configs.len());
    for cfg in configs {
        if let Some(s) = convert_one(cfg, caps) {
            out.push(s);
        }
    }
    out
}

fn convert_one(cfg: &McpConfig, caps: &McpCapabilities) -> Option<McpServer> {
    match cfg {
        McpConfig::Stdio(s) => {
            if s.command.trim().is_empty() {
                return None;
            }
            let stdio = McpServerStdio::new(s.name.clone(), PathBuf::from(s.command.clone()))
                .args(s.args.clone());
            Some(McpServer::Stdio(stdio))
        }
        McpConfig::StreamableHttp(h) => {
            if !caps.http {
                return None;
            }
            if h.endpoint.trim().is_empty() {
                return None;
            }
            let headers = match &h.auth {
                McpAuthConfig::None => Vec::new(),
                McpAuthConfig::BearerToken { token } if !token.is_empty() => {
                    vec![HttpHeader::new(
                        "Authorization",
                        format!("Bearer {token}"),
                    )]
                }
                McpAuthConfig::BearerToken { .. } => Vec::new(),
                // OAuth2 tokens live in Ergon's encrypted store and are
                // not forwarded to subprocess agents in this version.
                McpAuthConfig::OAuth2 { .. } => return None,
            };
            let http = McpServerHttp::new(h.name.clone(), h.endpoint.clone()).headers(headers);
            Some(McpServer::Http(http))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpStdioConfig, McpStreamableHttpConfig};

    fn caps(http: bool, sse: bool) -> McpCapabilities {
        McpCapabilities::new().http(http).sse(sse)
    }

    #[test]
    fn stdio_always_included() {
        let cfgs = vec![McpConfig::Stdio(McpStdioConfig {
            name: "fs".into(),
            command: "/usr/bin/mcp-fs".into(),
            args: vec!["--root".into(), "/tmp".into()],
        })];
        let out = mcp_servers_from_configs(&cfgs, &caps(false, false));
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], McpServer::Stdio(_)));
    }

    #[test]
    fn stdio_with_empty_command_dropped() {
        let cfgs = vec![McpConfig::Stdio(McpStdioConfig {
            name: "x".into(),
            command: "   ".into(),
            args: vec![],
        })];
        assert!(mcp_servers_from_configs(&cfgs, &caps(true, true)).is_empty());
    }

    #[test]
    fn http_gated_by_capability() {
        let cfgs = vec![McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "remote".into(),
            endpoint: "https://mcp.example.com".into(),
            auth: McpAuthConfig::None,
        })];
        assert!(mcp_servers_from_configs(&cfgs, &caps(false, false)).is_empty());
        let out = mcp_servers_from_configs(&cfgs, &caps(true, false));
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], McpServer::Http(_)));
    }

    #[test]
    fn http_bearer_token_becomes_authorization_header() {
        let cfgs = vec![McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "remote".into(),
            endpoint: "https://mcp.example.com".into(),
            auth: McpAuthConfig::BearerToken {
                token: "secret".into(),
            },
        })];
        let out = mcp_servers_from_configs(&cfgs, &caps(true, false));
        let McpServer::Http(h) = &out[0] else {
            panic!("expected http variant")
        };
        assert_eq!(h.headers.len(), 1);
        assert_eq!(h.headers[0].name, "Authorization");
        assert_eq!(h.headers[0].value, "Bearer secret");
    }

    #[test]
    fn http_oauth2_dropped() {
        let cfgs = vec![McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "remote".into(),
            endpoint: "https://mcp.example.com".into(),
            auth: McpAuthConfig::OAuth2 {
                scopes: vec![],
                client_name: "Ergon".into(),
                redirect_port: 8585,
            },
        })];
        assert!(mcp_servers_from_configs(&cfgs, &caps(true, true)).is_empty());
    }
}
