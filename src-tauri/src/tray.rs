//! Tauri tray-icon construction: tooltip, left-click disabled, right-click menu.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

pub fn build_tray(app: &AppHandle) -> tauri::Result<TrayIcon<Wry>> {
    let show_curve = MenuItem::with_id(app, "show_curve", "View Power Curve", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_curve, &quit])?;

    let icon = TrayIconBuilder::with_id("main")
        .icon(default_tray_icon())
        .tooltip("BatteryShower")
        .menu(&menu)
        .show_menu_on_left_click(false) // 左键单击不弹菜单（rule #5）
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show_curve" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|_tray, event| {
            // 显式吃掉左键单击（rule #5: 左键不响应）
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // noop
            }
        })
        .build(app)?;
    Ok(icon)
}

fn default_tray_icon() -> tauri::image::Image<'static> {
    // 64×64 neutral gray placeholder — visible against any Windows theme,
    // replaced within ~1 s of the first battery sample. PNG-encoded so
    // it goes through the same `Image::from_bytes` path as the live icon.
    use tauri::image::Image as TauriImage;
    let png = crate::render::default_icon();
    TauriImage::from_bytes(&png).unwrap_or_else(|_| {
        // PNG decode failed — fall back to a tiny 1×1 transparent icon.
        // (Never reached unless render::default_icon is broken.)
        TauriImage::new_owned(vec![0u8; 4], 1, 1)
    })
}
