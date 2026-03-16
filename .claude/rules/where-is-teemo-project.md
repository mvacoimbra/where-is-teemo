# Project: Where Is Teemo

**Last Updated:** 2026-03-16

## Overview

Cross-platform Tauri v2 desktop app that appears offline in League of Legends and VALORANT by proxying XMPP chat traffic. System tray app with no dock icon.

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 |
| Frontend | React 19, TypeScript 5.8, Vite 7 |
| Backend | Rust (tokio async runtime) |
| TLS | rustls 0.23 + tokio-rustls 0.26 |
| Certs | rcgen 0.14 |
| HTTP Proxy | hyper 1 + reqwest 0.12 |
| Process Mgmt | sysinfo 0.35 |
| macOS native | objc2 0.6 + block2 0.6 |
| Package Manager | Yarn v1, Cargo |

## Directory Structure

```
src/                    # Frontend (React + TypeScript)
  App.tsx               # Main UI component (single page)
  types.ts              # TypeScript types matching Rust serde structs
src-tauri/              # Backend (Rust + Tauri v2)
  src/
    lib.rs              # App setup, tray icon, window management
    commands.rs         # Tauri IPC command handlers
    state.rs            # AppState with Mutex<AppStateInner>
    proxy/
      mod.rs            # ProxyHandle, start_proxy()
      certs.rs          # CA & server cert generation (rcgen)
      config_proxy.rs   # HTTP proxy for Riot client config
      xmpp_proxy.rs     # TLS XMPP proxy with stanza filtering
      presence.rs       # Presence stanza parser & filter
    riot/
      config.rs         # Region-to-chat-server mapping (16 regions)
      process.rs        # Riot client detection & launch (macOS/Windows)
```

## Development Commands

| Task | Command |
|------|---------|
| Install deps | `yarn install` |
| Dev (frontend + backend) | `yarn tauri dev` |
| Build frontend only | `yarn build` (= `tsc && vite build`) |
| Build production app | `yarn tauri build` |
| Run Rust tests | `cd src-tauri && cargo test` |
| Rust check | `cd src-tauri && cargo check` |

## Commit Rules

- Conventional commits: `type: description` (max 72 chars)
- Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
- No scope, no body, no footer, no co-authorship
- Group related changes into separate commits by type

## UI Language

The frontend UI uses **Brazilian Portuguese** for all user-facing strings. Keep this consistent when modifying UI text.

## CI/CD

- GitHub Actions release workflow on `v*` tags
- Builds macOS (universal binary) and Windows
- Uses `tauri-apps/tauri-action@v0` for cross-platform builds
- Releases are created as drafts with auto-generated notes
