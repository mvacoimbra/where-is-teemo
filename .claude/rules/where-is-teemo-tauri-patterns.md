# Tauri Patterns

## IPC Commands

All Tauri commands live in `src-tauri/src/commands.rs`. Pattern:

```rust
#[tauri::command]
pub fn command_name(state: State<'_, AppState>) -> ReturnType { ... }

// Async commands:
#[tauri::command]
pub async fn command_name(app: AppHandle, state: State<'_, AppState>) -> Result<T, String> { ... }
```

Register in `lib.rs` via `tauri::generate_handler![...]`. Frontend calls via `invoke<T>("command_name", { args })`.

**IMPORTANT:** Return types must derive `serde::Serialize`. Error type is `String` (not custom error enums).

### Current Commands

| Command | Sync/Async | Returns |
|---------|-----------|---------|
| `get_status` | sync | `StatusInfo` |
| `set_stealth_mode` | sync | `StatusInfo` |
| `launch_game` | async | `Result<StatusInfo, String>` |
| `stop_proxy` | sync | `StatusInfo` |
| `get_cert_status` | sync | `Result<CertStatus, String>` |
| `install_ca` | sync | `Result<(), String>` |
| `get_regions` | sync | `Vec<RegionInfo>` |
| `set_region` | sync | `Result<(), String>` |

## State Management

`AppState` wraps `Mutex<AppStateInner>`. Access pattern:

```rust
// Read
let inner = state.inner.lock().unwrap();

// Write
let mut inner = state.inner.lock().unwrap();
inner.field = value;
```

Channels for runtime communication:
- `mode_tx: watch::Sender<StealthMode>` — toggle stealth mode while proxy runs
- `shutdown_tx / config_shutdown_tx` — graceful proxy shutdown
- `host_tx` — update XMPP target when real chat host is discovered

## Tray Icon

- System tray with right-click context menu (Invisible, Online, Show Window, Quit)
- Left-click toggles popover window positioned below tray icon
- Icon changes based on OS theme (dark/light)
- Window is non-resizable, 380x480, no decorations, no taskbar, always on top

## macOS-Specific

- Click-outside handler via `objc2` global event monitor (`NSEvent addGlobalMonitorForEventsMatchingMask`)
- Window close is intercepted — hides instead of closing
- Riot Client launched via `open -a` with `--args`

## Platform Guards

Use `#[cfg(target_os = "macos")]` / `#[cfg(target_os = "windows")]` for platform-specific code. Always provide a fallback `#[cfg(not(any(...)))]` block.
