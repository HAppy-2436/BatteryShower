//! BatteryShower — Tauri 2 + Rust + Vue 3 entry point.
//!
//! Lifecycle:
//! 1. `setup` — open SQLite store, init battery monitor, build tray, start
//!    1Hz sampling loop on a tokio task.
//! 2. Sampling loop — read battery, persist sample, render tray icon, update
//!    tooltip, emit `battery-update` to the frontend.
//! 3. Tauri commands — let the Vue UI fetch stored charge/discharge sessions
//!    for the curve window.

mod color;
mod estimator;
mod render;
mod sensors;
mod store;
mod tray;

use std::sync::Arc;
use std::time::Duration;

use tauri::async_runtime;
use tauri::image::Image;
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::estimator::{estimate_remaining_seconds, format_remaining, PowerAverage};
use crate::render::render_icon;
use crate::sensors::battery::BatteryReading;
use crate::sensors::State;
use crate::store::{Sample, Session, Store};

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    monitor: Arc<sensors::battery::BatteryMonitor>,
    /// id of the currently active session, if any (for "only-keep-most-recent-of-each-state")
    current_session: Arc<Mutex<Option<CurrentSession>>>,
    power_avg: Arc<Mutex<PowerAverage>>,
}

#[derive(Clone, Debug)]
struct CurrentSession {
    id: i64,
    state: String,
}

#[tauri::command]
fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn get_latest_charge_session(state: tauri::State<'_, AppState>) -> Result<Option<Session>, String> {
    state
        .store
        .get_latest_session("charging")
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_latest_discharge_session(state: tauri::State<'_, AppState>) -> Result<Option<Session>, String> {
    state
        .store
        .get_latest_session("discharging")
        .map_err(|e| e.to_string())
}

/// Public so the Vue UI can display whatever the current sample says.
#[derive(serde::Serialize, Clone)]
struct BatterySnapshot {
    state: String,
    percentage: u8,
    power_watts: f64,
    voltage_v: f64,
    current_a: f64,
    full_charge_capacity_mwh: u32,
    avg_power_w: Option<f64>,
    remaining_sec: Option<i64>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();
            let db_path = app_dir.join("battery.sqlite");
            let store = Store::open(db_path.to_str().unwrap())
                .expect("open SQLite store");

            let tray_icon = tray::build_tray(&app.handle())?;

            let state = AppState {
                store: Arc::new(store),
                monitor: Arc::new(sensors::battery::BatteryMonitor::new()),
                current_session: Arc::new(Mutex::new(None)),
                power_avg: Arc::new(Mutex::new(PowerAverage::new(300))), // 5 min sliding window
            };
            app.manage(state);

            start_sampling_loop(app.handle().clone(), tray_icon);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            get_latest_charge_session,
            get_latest_discharge_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn start_sampling_loop(app: AppHandle, tray: TrayIcon) {
    async_runtime::spawn(async move {
        let state = app.state::<AppState>().inner().clone();
        loop {
            if let Some(reading) = state.monitor.read() {
                process_reading(&app, &state, &tray, reading).await;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn process_reading(
    app: &AppHandle,
    state: &AppState,
    tray: &TrayIcon,
    reading: BatteryReading,
) {
    let state_label = match reading.state {
        State::Charging => "charging",
        State::Discharging => "discharging",
        State::Full => "full",
    };

    // ---- 1. Manage session lifecycle (rule #7: only-most-recent of each state) ----
    {
        let mut current = state.current_session.lock().await;
        let need_new_session = match &*current {
            // Already tracking this exact state → keep
            Some(c) => c.state == state_label,
            // Not tracking, and we're not Full → start a new one
            None => state_label != "full",
            // Tracking some other state, and we're not Full → start a new one
            // (Note: "Full" doesn't get its own session; the latest charge/discharge session persists.)
        };
        if need_new_session {
            // Close existing session (if any)
            if let Some(old) = current.take() {
                let _ = state.store.end_session(old.id);
            }
            if state_label != "full" {
                if let Ok(id) = state.store.start_new_session(state_label) {
                    *current = Some(CurrentSession {
                        id,
                        state: state_label.to_string(),
                    });
                }
            }
        }
    }

    // ---- 2. Persist this sample to the active session ----
    if let Some(c) = state.current_session.lock().await.clone() {
        let sample = Sample {
            timestamp: chrono::Utc::now().timestamp(),
            state: state_to_int(reading.state),
            power_watts: reading.power_watts,
            percentage: reading.percentage as i32,
        };
        let _ = state.store.insert_sample(c.id, &sample);
    }

    // ---- 3. Sliding-window power average ----
    let now_ts = chrono::Utc::now().timestamp();
    let mut avg = state.power_avg.lock().await;
    avg.push(now_ts, reading.power_watts);
    let avg_power = avg.avg();
    drop(avg);

    // ---- 4. Update tray icon (rule #1: full→white, charging→green, discharge→red; rule #1.1 gradient) ----
    if reading.state != State::Full {
        let display = format!("{:.1}", reading.power_watts);
        let pixels = render_icon(&display, reading.state, reading.percentage);
        let img = Image::new_owned(pixels, 32, 32);
        let _ = tray.set_icon(Some(img));
    } else {
        // Full → empty icon (rule #2.1: hover shows nothing)
        let img = Image::new_owned(render_icon("", State::Full, 100), 32, 32);
        let _ = tray.set_icon(Some(img));
    }

    // ---- 5. Update tooltip (rule #2: % + remaining; rule #2.1: full→nothing) ----
    let tooltip = build_tooltip(reading, avg_power, reading.full_charge_capacity_mwh);
    let _ = tray.set_tooltip(Some(&tooltip));

    // ---- 6. Emit to frontend ----
    let snapshot = BatterySnapshot {
        state: state_label.to_string(),
        percentage: reading.percentage,
        power_watts: reading.power_watts,
        voltage_v: reading.voltage_v,
        current_a: reading.current_a,
        full_charge_capacity_mwh: reading.full_charge_capacity_mwh,
        avg_power_w: avg_power,
        remaining_sec: compute_remaining(reading, avg_power),
    };
    let _ = app.emit("battery-update", &snapshot);
}

fn state_to_int(state: State) -> i32 {
    match state {
        State::Charging => 0,
        State::Discharging => 1,
        State::Full => 2,
    }
}

fn build_tooltip(reading: BatteryReading, avg_power: Option<f64>, full_mwh: u32) -> String {
    if reading.state == State::Full {
        return "BatteryShower | 已充满".to_string();
    }
    let label = match reading.state {
        State::Charging => "充电中",
        State::Discharging => "放电中",
        State::Full => "已充满",
    };
    let remain_str = match compute_remaining(reading, avg_power) {
        Some(sec) => format_remaining(sec),
        None => "—".to_string(),
    };
    let _ = full_mwh;
    format!("{} | {}% | 剩余 {}", label, reading.percentage, remain_str)
}

fn compute_remaining(reading: BatteryReading, avg_power: Option<f64>) -> Option<i64> {
    let avg = avg_power?;
    if avg.abs() < 0.05 {
        return None;
    }
    if reading.full_charge_capacity_mwh == 0 {
        return None;
    }
    let current_mwh =
        reading.full_charge_capacity_mwh as f64 * (reading.percentage as f64 / 100.0);
    Some(estimate_remaining_seconds(
        current_mwh,
        reading.full_charge_capacity_mwh as f64,
        avg,
    ))
}
