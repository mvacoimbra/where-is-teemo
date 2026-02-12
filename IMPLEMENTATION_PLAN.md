# Riot Stealth — Implementation Plan

## Context

This is a **Tauri v2** app (Rust backend + React/TypeScript frontend) that acts as a local XMPP proxy to make the user appear offline in League of Legends and VALORANT. It's inspired by [Deceive](https://github.com/molenzwiebel/Deceive) (C#/Windows-only), but cross-platform (macOS + Windows).

The scaffold is already created. The project structure exists with placeholder implementations. Your job is to make each component actually work, following the phases below in order.

## Tech Stack

- **Frontend**: React 18, TypeScript (no interfaces — use `type` only), Vite
- **Backend**: Rust (Tauri v2)
- **Package manager**: Yarn
- **Target platforms**: macOS, Windows

## Project Structure (already exists)

```
riot-stealth/
├── src/                          # React frontend
│   ├── App.tsx                   # Main UI — toggle + game launcher
│   ├── main.tsx                  # Entry point
│   ├── app.css                   # Styles (dark theme, already done)
│   └── types.ts                  # Shared TS types
├── src-tauri/
│   ├── Cargo.toml                # Rust deps (already configured)
│   ├── tauri.conf.json           # Tauri config
│   └── src/
│       ├── main.rs               # Tauri entry + tray setup
│       ├── lib.rs                # Module declarations
│       ├── commands.rs           # IPC commands (FE ↔ BE)
│       ├── state.rs              # Shared app state (stealth mode, proxy status)
│       ├── proxy/
│       │   ├── mod.rs            # Proxy orchestrator
│       │   ├── config_proxy.rs   # Riot config patching
│       │   ├── xmpp_proxy.rs     # TLS MITM proxy
│       │   └── presence.rs       # XMPP presence stanza filtering
│       └── riot/
│           ├── mod.rs            # Game enum + helpers
│           ├── process.rs        # Find/kill/launch Riot client
│           └── config.rs         # Region detection + chat server addresses
├── package.json
├── tsconfig.json
├── vite.config.ts
└── ARCHITECTURE.md               # Detailed architecture doc — READ THIS FIRST
```

## How It Works (high level)

```
Riot Client  --TLS-->  Riot Stealth (localhost:5223)  --TLS-->  Riot Chat Server
                              |
                     Intercepts <presence> stanzas
                     and replaces with "unavailable"
```

1. User clicks "Launch LoL" or "Launch VALORANT"
2. App kills any existing Riot processes
3. App patches the Riot client config to redirect `chat_host` to `127.0.0.1`
4. App starts a local TLS server on port 5223
5. App launches the Riot client
6. Riot client connects to `127.0.0.1:5223` (our proxy) thinking it's the real chat server
7. Proxy opens a real TLS connection to the actual Riot chat server
8. All traffic is forwarded transparently, EXCEPT outgoing `<presence>` stanzas which get modified to show "offline"
9. User can toggle between offline/online at any time via the UI

---

## Phase 1: Build & Dev Environment

**Goal**: Make the project compile and run with `yarn tauri dev`.

### Tasks

1. Run `yarn install` and verify frontend deps resolve
2. Run `cargo check` inside `src-tauri/` and fix any compilation errors
3. Make sure `yarn tauri dev` opens the window with the React UI
4. The UI should render (toggle + game buttons) even though nothing works yet — the backend commands should return mock/default data without crashing

### Acceptance Criteria
- `yarn tauri dev` opens the app window
- No Rust compilation errors
- Frontend renders the toggle and game buttons
- `get_status` command returns the default idle state

---

## Phase 2: TLS Certificate Generation & Trust

**Goal**: Generate a self-signed CA at first launch and install it in the OS trust store.

### Context
The Riot client uses TLS to connect to chat servers. Our proxy sits in the middle, so the Riot client will connect to us over TLS. It will reject a self-signed cert unless we install our CA in the system trust store.

### Tasks

1. **Generate a root CA certificate** using `rcgen` on first launch
   - Store the CA cert + key in the app's data directory (`tauri::api::path::app_data_dir`)
   - If the CA already exists on disk, load it instead of regenerating
   - Use the CA to sign a server cert for `127.0.0.1` and `localhost`

2. **Install the CA in the OS trust store**
   - **macOS**: Run `security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <cert.pem>` (requires sudo/admin prompt)
   - **Windows**: Run `certutil -addstore -user "Root" <cert.pem>` or use the `winapi` crate to add to the cert store programmatically
   - Show a dialog to the user explaining why admin access is needed
   - Only do this once — check if cert is already installed before prompting

3. **Create a `certs` module** in `src-tauri/src/proxy/certs.rs`
   - `fn ensure_ca() -> Result<(CertificateDer, PrivateKeyDer)>` — load or generate CA
   - `fn generate_server_cert(ca: &...) -> Result<ProxyCerts>` — sign a cert for localhost
   - `fn install_ca_system(ca_pem: &[u8]) -> Result<()>` — OS-specific trust store install
   - `fn is_ca_installed() -> bool` — check if already trusted

### Acceptance Criteria
- First launch generates CA + server cert in the app data dir
- Second launch reuses existing CA
- CA is installed in the system trust store (user is prompted for admin)
- A TLS server using the generated cert can be connected to from `curl --cacert` or a browser without warnings

---

## Phase 3: XMPP Proxy Core

**Goal**: Get the bidirectional TLS proxy working — accepting connections on localhost and forwarding to a real server.

### Context
File `src-tauri/src/proxy/xmpp_proxy.rs` has the skeleton. The key pieces:
- Local TLS server on `127.0.0.1:5223` using the certs from Phase 2
- Connect to the real Riot chat server (e.g. `br1.chat.si.riotgames.com:5223`) over TLS
- Bidirectional forwarding with `tokio::io::split` + two tasks

### Tasks

1. **Refactor `xmpp_proxy.rs`** to use the CA-signed certs from Phase 2 instead of generating throwaway certs
2. **Test the raw proxy** (before presence filtering):
   - Start the proxy
   - Use an XMPP client (or `openssl s_client`) to connect to `localhost:5223`
   - Verify it can complete TLS handshake and forward data to/from the real server
3. **Handle edge cases**:
   - XMPP stanzas can be split across multiple TCP reads. Buffer incoming data and only process complete stanzas (look for matching close tags). This is critical — partial stanza modification will corrupt the stream.
   - Handle connection drops gracefully (Riot client reconnects, server disconnects)
   - Handle multiple simultaneous connections (Riot may open more than one)
4. **Wire up presence filtering** in the client→server direction:
   - Use `presence::filter_outgoing()` on the data before forwarding to the real server
   - The `mode_rx` watch channel lets the proxy react to stealth toggle changes in real-time without reconnecting

### Important: Stanza Buffering

XMPP stanzas may arrive split across TCP reads. The current code treats each `read()` as a complete message — this WILL break. Implement a simple stanza buffer:

```rust
// Pseudocode for the client→server direction:
let mut buffer = String::new();
loop {
    let n = client_read.read(&mut buf).await?;
    buffer.push_str(&String::from_utf8_lossy(&buf[..n]));
    
    // Process all complete stanzas in the buffer
    while let Some(stanza_end) = find_stanza_end(&buffer) {
        let stanza = buffer.drain(..stanza_end).collect::<String>();
        let filtered = presence::filter_outgoing(&stanza, &mode);
        server_write.write_all(filtered.as_bytes()).await?;
    }
}
```

You don't need a full XML parser. Stanzas end with `</presence>`, `</message>`, `</iq>`, etc. Track open/close tags with a simple counter or look for known closing tags.

### Acceptance Criteria
- Proxy accepts TLS connections on localhost:5223
- Data flows bidirectionally to the real Riot chat server
- Presence stanzas are correctly modified when stealth is "offline"
- Non-presence stanzas pass through unmodified
- Split stanzas are handled correctly (buffered until complete)

---

## Phase 4: Riot Client Config Patching & Process Management

**Goal**: Automatically redirect the Riot client's chat connection to our proxy.

### Context
The Riot client determines which chat server to connect to from its configuration. We need to intercept this. Deceive does it by patching a YAML config file on disk.

### Tasks

1. **Research the actual Riot client config location and format**
   - The Riot client fetches config from `https://clientconfig.rpg.riotgames.com/api/v1/config/player`
   - But it also caches/stores settings locally. Find where.
   - On Windows, check: `%LOCALAPPDATA%\Riot Games\Riot Client\Config\`, and `C:\Riot Games\Riot Client\Data\`
   - On macOS, check: `~/Library/Application Support/Riot Games/`
   - **Alternative approach (preferred by Deceive)**: Instead of patching config files, Deceive modifies the system's YAML config that tells the Riot client which chat server to use. Specifically, it patches the `chat_host` value in the player config. But the more reliable approach is to **launch the Riot client with `--client-config-url` pointing to a local HTTP server** that serves a modified config.

2. **Implement the config interception strategy**. Pick ONE of these:

   **Option A — Local Config HTTP Server (recommended, most reliable)**:
   - Start a local HTTPS server (e.g. `127.0.0.1:5380`)
   - When the Riot client asks for config, proxy the request to the real config endpoint
   - In the response JSON, replace the `chat_host` / `chat.host` value with `127.0.0.1`
   - Launch the Riot client with: `RiotClientServices.exe --client-config-url="https://127.0.0.1:5380"`
   - This is what Deceive actually does internally

   **Option B — Direct config file patching (simpler but fragile)**:
   - Find the cached config YAML/JSON on disk
   - Replace the chat host value
   - Launch the Riot client normally
   - Restore the original config on app exit

3. **Process management** (`riot/process.rs`):
   - `kill_riot_client()` — already implemented, verify it works on both OSes
   - `launch_game(game)` — needs to launch with the proxy flag
   - `find_riot_client()` — verify the paths are correct for both OSes
   - Add a `is_riot_running()` check before killing

4. **Restore on exit**: When Riot Stealth closes, restore the original config. Use Tauri's `on_window_event` with `CloseRequested` or `RunEvent::ExitRequested` to run cleanup.

### Acceptance Criteria
- Riot client launches and connects to our proxy instead of the real chat server
- The real chat server address is extracted from config (for our proxy to connect to)
- Config is restored when the app exits
- Works on both macOS and Windows

---

## Phase 5: Region Detection

**Goal**: Automatically detect the player's region so we connect to the right chat server.

### Context
Each Riot region has its own chat server:
- BR → `br1.chat.si.riotgames.com`
- NA → `na2.chat.si.riotgames.com`
- EUW → `euw1.chat.si.riotgames.com`
- etc.

### Tasks

1. **Primary detection**: Parse the Riot client's config to find the current chat server address. If we're using Option A from Phase 4 (config proxy), we already see the real chat host in the config response — just extract it.

2. **Fallback detection**: Parse `RiotClientInstalls.json` or the player's `settings.yaml` for region hints.

3. **Manual override**: Add a region selector dropdown in the frontend that saves to the Tauri app's local storage. Only show it if auto-detection fails.

4. **Store the detected region** in `AppState` so the proxy knows which server to connect to.

### Acceptance Criteria
- Region is auto-detected on launch
- Proxy connects to the correct regional chat server
- User can manually override region if needed

---

## Phase 6: System Tray

**Goal**: Add a system tray icon so the app can run minimized.

### Tasks

1. **Setup tray icon** in `main.rs` using Tauri v2's `TrayIconBuilder`
2. **Tray menu items**:
   - "Offline" (checked when stealth is active)
   - "Online"
   - Separator
   - "Show Window"
   - "Quit"
3. **Minimize to tray** on window close (don't actually quit):
   - Handle `CloseRequested` event → hide window instead of closing
   - Double-click tray icon → show window
4. **Tray icon changes based on state**:
   - Grey icon when proxy is idle
   - Colored/active icon when proxy is running and stealth is on

### Acceptance Criteria
- Closing the window minimizes to tray
- Tray icon shows current status
- Can toggle stealth mode from tray menu
- "Quit" properly shuts down proxy and restores config

---

## Phase 7: Error Handling & Polish

**Goal**: Make the app robust and user-friendly.

### Tasks

1. **Error handling**:
   - If Riot client is not installed → show clear error message with install link
   - If port 5223 is already in use → show error, offer to kill the process using it
   - If TLS handshake fails → log details, show user-friendly error
   - If Riot updates break the config format → graceful fallback

2. **Logging**:
   - Write logs to the app data dir (use `env_logger` or `tracing`)
   - Frontend shows a collapsible log viewer for debugging
   - Log all intercepted presence stanzas (in debug mode only)

3. **Auto-update check**:
   - On launch, check GitHub releases for a newer version
   - Show a non-intrusive notification if an update is available

4. **Frontend polish**:
   - Add a connection status animation (pulsing dot when connecting)
   - Show which game is currently running
   - Add a "Reconnect" button if the proxy connection drops
   - Add tooltips explaining what each button does

5. **Cleanup on crash**: Register a panic hook that restores Riot config even if the app crashes.

### Acceptance Criteria
- No unhandled panics
- Logs are written to disk
- User sees actionable error messages
- Config is always restored, even on crash

---

## Implementation Notes

### Things to watch out for

1. **Riot Vanguard (VALORANT's anti-cheat)**: Vanguard runs at the kernel level. Our proxy does NOT modify game memory or inject code — it's purely a network proxy. This is the same approach Deceive uses, and Riot has confirmed it won't result in a ban. However, Vanguard updates could potentially interfere with the proxy approach.

2. **XMPP stream initialization**: The XMPP connection starts with `<stream:stream>` which is NOT a self-closing tag — it wraps the entire session. Don't try to match it as a stanza. Only filter `<presence>`, `<message>`, and `<iq>` stanzas.

3. **TLS SNI**: The Riot client may check SNI (Server Name Indication) during TLS handshake. Our local server cert should include the original chat server hostname as a SAN, or the client might reject the connection.

4. **Multiple connections**: The Riot client may open multiple XMPP connections (e.g., one for LoL, one for VALORANT). The proxy should handle concurrent connections.

5. **Presence stanza format varies by game**: LoL and VALORANT use slightly different presence XML. Test both. The key is that ALL stanzas starting with `<presence` need to be filtered, regardless of their internal structure.

6. **UTF-8**: XMPP stanzas are UTF-8. Player names, status messages, etc. may contain non-ASCII characters. Always handle encoding properly.

7. **macOS specific**: macOS may require the app to be signed for the TLS cert installation to work properly. During development, you may need to manually trust the cert in Keychain Access.

### Useful references

- [Deceive source code](https://github.com/molenzwiebel/Deceive) — The C# reference implementation. Pay special attention to `ProxiedConnection.cs` and `StartupHandler.cs`.
- [Riot XMPP chat architecture](https://technology.riotgames.com/news/chat-service-architecture-protocol) — Official Riot engineering blog about their XMPP implementation.
- [Tauri v2 docs](https://v2.tauri.app/) — For Tauri-specific APIs.
- [XMPP RFC 6120](https://datatracker.ietf.org/doc/html/rfc6120) — Core XMPP protocol spec (for understanding stanza structure).

### Testing approach

Since this involves network proxying and process management, automated testing is limited. Focus on:

- **Unit tests** for `presence.rs` stanza filtering (already has some)
- **Unit tests** for config parsing and region detection
- **Manual integration testing** with the actual Riot client
- **Use `openssl s_client`** to test the TLS proxy independently:
  ```bash
  openssl s_client -connect 127.0.0.1:5223 -servername br1.chat.si.riotgames.com
  ```
