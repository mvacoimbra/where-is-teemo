# Proxy Architecture

## Overview

```
Riot Client ──HTTP──> localhost:random (Config Proxy) ──HTTPS──> clientconfig.rpg.riotgames.com
Riot Client ──TLS───> localhost:5223   (XMPP Proxy)   ──TLS───> {region}.chat.si.riotgames.com
```

## Launch Flow (`commands::launch_game`)

1. Kill existing Riot processes (`sysinfo`)
2. Generate/load CA + server certs (`rcgen`)
3. Start config proxy on random port (HTTP, intercepts Riot config)
4. Start XMPP proxy on port 5223 (TLS, filters presence stanzas)
5. Launch Riot Client with `--client-config-url=http://127.0.0.1:{port}`
6. Background task updates XMPP proxy target when real chat host discovered

## Config Proxy (`proxy::config_proxy`)

- HTTP server on `127.0.0.1:0` (random port)
- Forwards requests to `https://clientconfig.rpg.riotgames.com`
- Patches JSON responses: replaces `chat.host` with `127.0.0.1`, `chat.port` with 5223, all `chat.affinities` with localhost
- Extracts real chat host and sends via `watch` channel
- Only forwards headers: `user-agent`, `x-riot-entitlements-jwt`, `authorization`

## XMPP Proxy (`proxy::xmpp_proxy`)

- TLS server on `127.0.0.1:5223` using locally-generated server cert
- Accepts TLS from Riot client, connects TLS to real Riot chat server
- Bidirectional: server-to-client passes through unmodified
- Client-to-server: filters `<presence>` stanzas based on stealth mode
- On mode toggle: injects presence stanza (unavailable or cached last presence)

## Presence Filtering (`proxy::presence`)

**When offline:** `<presence>` stanzas rewritten to `type="unavailable"`, body stripped. All other stanzas pass through.

**Stanza boundary detection:** `find_stanza_end()` handles:
- XML declarations (`<?xml ... ?>`)
- Self-closing tags (`<presence ... />`) — quote-aware to avoid child `/>` confusion
- Full tags with closing (`<tag>...</tag>`)
- Stream-level opens (`<stream:stream>`)
- Closing tags (`</stream:stream>`)

**IMPORTANT:** Child self-closing elements (e.g., `<pty/>` inside `<presence>`) must NOT trigger premature stanza splitting. The parser only matches root-level `/>`.

## Certificate Chain

- CA cert: "Where Is Teemo CA" — generated once, stored in app data dir
- Server cert: signed by CA, SANs: `127.0.0.1` + `localhost`
- CA installed in OS trust store via `security` (macOS) or `certutil` (Windows)
- Certs stored at `{app_data_dir}/certs/`

## Channels (tokio::sync::watch)

| Channel | Sender Location | Receiver | Purpose |
|---------|----------------|----------|---------|
| `mode_tx/rx` | `AppState` | `xmpp_proxy` | Toggle stealth mode mid-session |
| `shutdown_tx/rx` | `AppState` | `xmpp_proxy` | Graceful XMPP proxy stop |
| `config_shutdown_tx/rx` | `AppState` | `config_proxy` | Graceful config proxy stop |
| `host_tx/rx` | `commands.rs` | `xmpp_proxy` | Update target host at runtime |
| `chat_host_tx/rx` | `config_proxy` | `commands.rs` | Real chat host discovery |
