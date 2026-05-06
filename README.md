# Ergon

[![Rust](https://github.com/milarze/ergon/actions/workflows/rust.yml/badge.svg)](https://github.com/milarze/ergon/actions/workflows/rust.yml)

An LLM chat interface built in Rust.

![Ergon](./ErgonChat.png)

## Features

- Models
  - Supports multiple LLMs
- Multi-modal
  - Text
  - Images
  - Audio
  - Files
- MCP
  - StreamableHTTP
  - STDIO
- Embedded models (TODO)
- Conversation management (TODO)
- ACP (Agent Client Protocol)
  - Spawn external agents over stdio
  - Streaming text, thoughts, and tool calls
  - Filesystem read/write callbacks (sandboxed to a configurable workspace root)
  - Terminal create / output / kill / wait
  - Permission requests
  - Authentication (`session/authenticate`) with inline sign-in buttons
  - Slash commands (`available_commands_update`) rendered as a chip row
  - Plan rendering (`plan` updates) with status + priority glyphs
  - Session resume (`session/load`) via a "Resume last session" button
  - MCP passthrough — Ergon's configured MCP servers are forwarded to the
    agent (stdio always; streamable HTTP gated on the agent's
    `mcp_capabilities.http`)

## Installation

```bash

cargo install ergon
```

## MCP

Ergon can host MCP servers over `stdio` or `StreamableHTTP`. Configure them in
**Settings → MCP Servers**:

- **Name** — used to identify the server in the chat-target picker.
- **Type** — stdio or Streamable HTTP.
- **Command + args** — how to spawn the server process (stdio servers only).
- **Endpoint** — the server's base URL (Streamable HTTP servers only).
- **Auth** — None, Bearer token, or OAuth2 (Streamable HTTP servers only).
- **Scopes** — OAuth2 scopes (Streamable HTTP servers with OAuth2 auth only).
- **Redirect Port** — port for receiving OAuth2 callbacks (Streamable HTTP
  servers with OAuth2 auth only).

## ACP agents

Ergon can act as an ACP *client* and drive an external agent process (e.g.
Claude Code, Gemini CLI, or your own implementation) over stdio. Configure
agents in **Settings → ACP Agents**:

- **Name** — used to identify the agent in the chat-target picker.
- **Command + args** — how to spawn the agent process.
- **Env** — literal `KEY=value, KEY=value` pairs injected into the child.
- **Workspace root** — directory used for filesystem sandboxing and as the
  session's `cwd`. Defaults to Ergon's working directory.

Once at least one agent is configured, the chat header gains a target picker
(LLM ↔ Agent). Selecting an agent spawns it (if not already running),
performs the ACP `initialize` handshake, and creates or resumes a session.

### Authentication

If an agent reports `auth_required`, Ergon renders a chat-bubble notice
listing the advertised methods and shows a row of "Sign in: \<Method\>"
buttons above the input. After authentication succeeds, session creation is
retried automatically.

### Session resume

When an agent advertises `agent_capabilities.load_session`, Ergon persists
the most recent session id (and its workspace root) under
`acp_session_state` in `~/.ergon/settings.json`. A **"Resume last session"**
button appears above the input whenever a stored session exists for the
selected agent. Clicking it issues `session/load`; if the stored workspace
root no longer matches, the resume is declined and a fresh session can be
created.

### MCP passthrough

Stdio MCP servers configured in Ergon are always forwarded to the agent.
Streamable-HTTP MCP servers are forwarded only if the agent advertises
`agent_capabilities.mcp_capabilities.http`. Bearer-token auth is converted
to an `Authorization: Bearer …` header. OAuth2-authed servers are not
forwarded (their tokens stay in Ergon).
