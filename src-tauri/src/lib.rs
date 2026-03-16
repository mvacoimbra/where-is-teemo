mod commands;
mod proxy;
mod riot;
mod state;

use state::AppState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_secs()
        .init();

    log::info!("Where Is Teemo starting");

    let app_state = AppState::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::set_stealth_mode,
            commands::launch_game,
            commands::stop_proxy,
            commands::get_cert_status,
            commands::install_ca,
            commands::get_regions,
            commands::set_region,
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            setup_certs(&data_dir);
            setup_tray(app)?;
            #[cfg(target_os = "macos")]
            setup_click_outside_handler(app);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::ThemeChanged(theme) => {
                if let Some(tray) = window.app_handle().tray_by_id("main-tray") {
                    let _ = tray.set_icon(Some(tray_icon_for_theme(*theme)));
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_certs(data_dir: &std::path::Path) {
    match proxy::certs::ensure_ca(data_dir) {
        Ok(ca) => {
            log::info!("CA certificate ready");
            if let Err(e) = proxy::certs::generate_server_cert(&ca, data_dir) {
                log::error!("Failed to generate server cert: {e}");
            }
        }
        Err(e) => {
            log::error!("Failed to ensure CA: {e}");
        }
    }
}

fn tray_icon_for_theme(theme: tauri::Theme) -> Image<'static> {
    match theme {
        tauri::Theme::Dark => {
            Image::from_bytes(include_bytes!("../icons/icon-colored-white.png")).unwrap()
        }
        _ => Image::from_bytes(include_bytes!("../icons/icon-colored-black.png")).unwrap(),
    }
}

#[cfg(target_os = "macos")]
fn setup_click_outside_handler(app: &tauri::App) {
    use block2::RcBlock;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::msg_send;
    use std::ptr::NonNull;

    let handle = app.handle().clone();

    // NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown
    let mask: u64 = (1 << 1) | (1 << 3);

    let block = RcBlock::new(move |_event: NonNull<AnyObject>| {
        if let Some(window) = handle.get_webview_window("main") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            }
        }
    });

    let cls = AnyClass::get(c"NSEvent").expect("NSEvent class not found");
    unsafe {
        let _: *mut AnyObject =
            msg_send![cls, addGlobalMonitorForEventsMatchingMask: mask, handler: &*block];
    }
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let offline_item = MenuItem::with_id(app, "offline", "Invisible", true, None::<&str>)?;
    let online_item = MenuItem::with_id(app, "online", "Online", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &offline_item,
            &online_item,
            &separator,
            &show_item,
            &quit_item,
        ],
    )?;

    let theme = app
        .get_webview_window("main")
        .and_then(|w| w.theme().ok())
        .unwrap_or(tauri::Theme::Dark);

    TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon_for_theme(theme))
        .tooltip("Where Is Teemo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "offline" => {
                let state = app.state::<AppState>();
                let mut inner = state.inner.lock().unwrap();
                inner.stealth_mode = state::StealthMode::Offline;
                if let Some(tx) = &inner.mode_tx {
                    let _ = tx.send(state::StealthMode::Offline);
                }
                log::info!("Stealth mode: Invisible (via tray)");
            }
            "online" => {
                let state = app.state::<AppState>();
                let mut inner = state.inner.lock().unwrap();
                inner.stealth_mode = state::StealthMode::Online;
                if let Some(tx) = &inner.mode_tx {
                    let _ = tx.send(state::StealthMode::Online);
                }
                log::info!("Stealth mode: Online (via tray)");
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                log::info!("Quit requested — cleaning up");
                let state = app.state::<AppState>();
                let mut inner = state.inner.lock().unwrap();
                if let Some(tx) = inner.shutdown_tx.take() {
                    let _ = tx.send(true);
                }
                if let Some(tx) = inner.config_shutdown_tx.take() {
                    let _ = tx.send(true);
                }
                drop(inner);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        position_window_near_tray(&window, rect.position, rect.size);
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn position_window_near_tray<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    pos: tauri::Position,
    size: tauri::Size,
) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let tray_pos = pos.to_logical::<f64>(scale);
    let tray_size = size.to_logical::<f64>(scale);

    #[cfg(target_os = "windows")]
    let (x, y) = {
        const W: f64 = 380.0;
        const H: f64 = 480.0;
        smart_position_windows(window, &tray_pos, &tray_size, scale, W, H)
            .unwrap_or_else(|| {
                // Fallback: above the tray icon
                (tray_pos.x + tray_size.width / 2.0 - W / 2.0, tray_pos.y - H)
            })
    };

    #[cfg(not(target_os = "windows"))]
    let (x, y) = (
        tray_pos.x + tray_size.width / 2.0 - 380.0_f64 / 2.0,
        tray_pos.y + tray_size.height,
    );

    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

/// Detects which edge of the screen the taskbar is on (by finding the edge
/// nearest to the tray icon) and positions the popup window on the opposite side.
#[cfg(target_os = "windows")]
fn smart_position_windows<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    tray_pos: &tauri::LogicalPosition<f64>,
    tray_size: &tauri::LogicalSize<f64>,
    scale: f64,
    window_w: f64,
    window_h: f64,
) -> Option<(f64, f64)> {
    // Convert tray center to physical pixels to match monitor coordinates
    let cx_phys = (tray_pos.x + tray_size.width / 2.0) * scale;
    let cy_phys = (tray_pos.y + tray_size.height / 2.0) * scale;

    let monitors = window.available_monitors().ok()?;

    // Find the monitor that contains the tray icon center
    let monitor = monitors
        .into_iter()
        .find(|m| {
            let p = m.position();
            let s = m.size();
            cx_phys >= p.x as f64
                && cx_phys < (p.x as f64 + s.width as f64)
                && cy_phys >= p.y as f64
                && cy_phys < (p.y as f64 + s.height as f64)
        })
        .or_else(|| window.primary_monitor().ok().flatten())?;

    let mon_pos = monitor.position().to_logical::<f64>(scale);
    let mon_size = monitor.size().to_logical::<f64>(scale);

    let tray_cx = tray_pos.x + tray_size.width / 2.0;
    let tray_cy = tray_pos.y + tray_size.height / 2.0;

    // Distance from the tray icon's outer edge to each screen edge
    let dist_top = tray_pos.y - mon_pos.y;
    let dist_bottom = (mon_pos.y + mon_size.height) - (tray_pos.y + tray_size.height);
    let dist_left = tray_pos.x - mon_pos.x;
    let dist_right = (mon_pos.x + mon_size.width) - (tray_pos.x + tray_size.width);

    let (mut x, mut y) = if dist_bottom <= dist_top.min(dist_left).min(dist_right) {
        // Taskbar at bottom → window appears above tray icon
        (tray_cx - window_w / 2.0, tray_pos.y - window_h)
    } else if dist_top <= dist_left.min(dist_right) {
        // Taskbar at top → window appears below tray icon
        (tray_cx - window_w / 2.0, tray_pos.y + tray_size.height)
    } else if dist_left <= dist_right {
        // Taskbar at left → window appears to the right
        (tray_pos.x + tray_size.width, tray_cy - window_h / 2.0)
    } else {
        // Taskbar at right → window appears to the left
        (tray_pos.x - window_w, tray_cy - window_h / 2.0)
    };

    // Clamp so the window never goes off screen
    x = x.clamp(mon_pos.x, mon_pos.x + mon_size.width - window_w);
    y = y.clamp(mon_pos.y, mon_pos.y + mon_size.height - window_h);

    Some((x, y))
}
