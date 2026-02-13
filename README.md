<p align="center">
  <img src="https://raw.githubusercontent.com/mvacoimbra/where-is-teemo/main/src-tauri/icons/128x128%402x.png" width="128" height="128" alt="Where Is Teemo icon" />
</p>

# Where Is Teemo?

> Nowhere to be found.

A cross-platform desktop app that lets you appear **offline** in League of Legends and VALORANT while still being able to play. Inspired by [Deceive](https://github.com/molenzwiebel/Deceive) (C#/Windows-only), built with [Tauri v2](https://v2.tauri.app/) for macOS and Windows.

## How It Works

Where Is Teemo acts as a local proxy between your Riot client and Riot's chat servers:

1. **Config Proxy** — An HTTP server on a random port intercepts the Riot client configuration request and rewrites the chat server address to `127.0.0.1`
2. **XMPP Proxy** — A TLS proxy on `localhost:5223` forwards all XMPP traffic to the real Riot chat server, but filters outgoing `<presence>` stanzas to make you appear offline
3. **Riot Client** is launched with `--client-config-url` pointing to the local config proxy

A locally-generated CA certificate is installed in your OS trust store so the Riot client accepts the TLS connection to the local proxy.

```
Riot Client ──TLS──▶ localhost:5223 (XMPP Proxy) ──TLS──▶ Riot Chat Server
                         │
                    filters <presence>
                    stanzas when stealth
                    mode is active
```

## Features

- **Appear offline** while playing League of Legends or VALORANT
- **System tray app** — lives in your status bar, no dock icon
- **One-click launch** — select a game and go
- **Auto region detection** — detects your chat server from Riot's config, with manual override for 16 regions
- **Toggle visibility** — switch between Invisible and Online at any time, even mid-game
- **Cross-platform** — macOS and Windows support

## Download

Grab the latest release for your platform from [GitHub Releases](https://github.com/mvacoimbra/where-is-teemo/releases):

- **macOS** — `.dmg` (universal binary: Apple Silicon + Intel)
- **Windows** — `.msi` installer or `.exe` (NSIS)

## Screenshots

The app runs as a popover anchored to your macOS status bar icon (or Windows system tray):

- Left-click the tray icon to toggle the panel
- Right-click for quick actions (Invisible, Online, Quit)

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (stable)
- [Yarn](https://yarnpkg.com/) v1

## Getting Started

```bash
# Clone the repo
git clone https://github.com/mvacoimbra/where-is-teemo.git
cd where-is-teemo

# Install frontend dependencies
yarn install

# Run in development mode
yarn tauri dev
```

On first launch, the app will:
1. Generate a CA certificate and server certificate
2. Prompt you to trust the CA in your OS keychain (requires admin password)

## Building for Production

```bash
yarn tauri build
```

The built app will be in `src-tauri/target/release/bundle/`.

## Project Structure

```
where-is-teemo/
├── src/                          # Frontend (React + TypeScript)
│   ├── App.tsx                   # Main UI component
│   ├── App.css                   # Dark theme styles
│   └── types.ts                  # TypeScript types
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── src/
│   │   ├── lib.rs                # App setup, tray, window management
│   │   ├── commands.rs           # Tauri IPC command handlers
│   │   ├── state.rs              # App state (stealth mode, proxy status)
│   │   ├── proxy/
│   │   │   ├── certs.rs          # CA & server certificate generation
│   │   │   ├── config_proxy.rs   # HTTP proxy for Riot client config
│   │   │   ├── xmpp_proxy.rs     # TLS XMPP proxy with stanza filtering
│   │   │   └── presence.rs       # Presence stanza parser & filter
│   │   └── riot/
│   │       ├── config.rs         # Region-to-chat-server mapping
│   │       └── process.rs        # Riot client detection & launch
│   ├── tauri.conf.json           # Tauri configuration
│   └── Cargo.toml                # Rust dependencies
└── package.json
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri v2](https://v2.tauri.app/) |
| Frontend | React 19, TypeScript, Vite 7 |
| Backend | Rust (tokio async runtime) |
| TLS | rustls + tokio-rustls |
| Cert Generation | rcgen 0.14 |
| HTTP Proxy | hyper 1 + reqwest |
| Process Management | sysinfo |

## How the XMPP Filtering Works

The proxy sits between the Riot client and Riot's chat server. All traffic passes through unchanged except for outgoing `<presence>` stanzas when stealth mode is active:

- **Online mode** — all stanzas pass through unmodified
- **Invisible mode** — `<presence>` stanzas are rewritten with `type="unavailable"`, stripping show/status elements so the server thinks you've disconnected from chat

The filtering handles both self-closing (`<presence ... />`) and full (`<presence>...</presence>`) XML formats, with proper stanza boundary detection to handle TCP stream fragmentation.

## Supported Regions

Auto-detected from Riot's config, or manually selectable:

BR, EUN, EUW, JP, KR, LA1, LA2, NA, OC, PH, RU, SG, TH, TR, TW, VN

## License

[MIT](LICENSE)
