//! Battery sensor — reads charge/discharge power, capacity, percentage.
//!
//! We use the `battery` crate rather than hand-rolling IOCTLs against
//! `\\.\BATTERY0`. Reason: BatteryMaster's reference implementation
//! uses the same crate, and on Windows `battery` enumerates devices via
//! SetupAPI (`GUID_DEVCLASS_BATTERY`), so it correctly finds the right
//! battery path — including `\\.\BATTERY1`, `\\.\CompositeBattery`, or
//! devices that aren't accessible from a non-Administrator session.
//!
//! IMPORTANT: `battery::Manager` is `!Send` (it wraps a `Rc` on Windows).
//! We therefore do NOT keep a Manager as a field — we build a fresh one
//! on every read(). The overhead is small (a single SetupDiGetClassDevs
//! call) and avoids needing a Mutex around the manager.
//!
//! Field reference: see topabomb/BatteryMaster crates/battery/src/battery_status.rs

use battery::units::electric_potential::volt;
use battery::units::energy::watt_hour;
use battery::units::power::watt;
use battery::units::ratio::percent;
use battery::{Manager, State as ExtState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Charging,
    Discharging,
    Full,
}

#[derive(Debug, Clone, Copy)]
pub struct BatteryReading {
    pub state: State,
    pub percentage: u8,
    /// Instantaneous power in watts (always >= 0).
    pub power_watts: f64,
    /// Design capacity in mWh. 0 if unknown.
    pub design_capacity_mwh: u32,
    /// Full-charge capacity in mWh. 0 if unknown.
    pub full_charge_capacity_mwh: u32,
    pub voltage_v: f64,
    pub current_a: f64,
}

pub struct BatteryMonitor;

impl BatteryMonitor {
    pub fn new() -> Self {
        // Probe once at startup so the log file gets a clear "yes/no battery
        // service available" line. Subsequent reads are per-call.
        match Manager::new() {
            Ok(m) => {
                let count = m.batteries().map(|it| it.count()).unwrap_or(0);
                crate::log::log_write(&format!(
                    "[BatteryShower:sensor] battery service OK, {} battery(ies) enumerated",
                    count
                ));
            }
            Err(e) => crate::log::log_write(&format!(
                "[BatteryShower:sensor] battery service unavailable: {}",
                e
            )),
        }
        Self
    }

    pub fn read(&self) -> Option<BatteryReading> {
        let manager = match Manager::new() {
            Ok(m) => m,
            Err(e) => {
                crate::log::log_write(&format!(
                    "[BatteryShower:sensor] Manager::new() failed: {}",
                    e
                ));
                return None;
            }
        };
        let mut iter = match manager.batteries() {
            Ok(it) => it,
            Err(e) => {
                crate::log::log_write(&format!(
                    "[BatteryShower:sensor] manager.batteries() failed: {}",
                    e
                ));
                return None;
            }
        };
        let battery = match iter.next() {
            Some(Ok(b)) => b,
            Some(Err(e)) => {
                crate::log::log_write(&format!(
                    "[BatteryShower:sensor] first battery entry error: {}",
                    e
                ));
                return None;
            }
            None => {
                // Not necessarily an error on a desktop with no battery;
                // log once at debug level via the absence of any later
                // successful read. The "no batteries found" line was already
                // emitted by new()'s count probe.
                return None;
            }
        };

        // Translate state — Unknown/Empty treated as Discharging (mirrors BM).
        let state = match battery.state() {
            ExtState::Charging => State::Charging,
            ExtState::Discharging => State::Discharging,
            ExtState::Full => State::Full,
            ExtState::Empty => State::Discharging,
            ExtState::Unknown => State::Discharging,
            _ => State::Discharging,
        };

        // uom 0.30's get::<U>() returns f32; we widen to f64 for our struct.
        let percentage = battery.state_of_charge().get::<percent>() as u8;
        let voltage_v: f64 = battery.voltage().get::<volt>().into();
        // energy_rate() is signed: positive = charging, negative = discharging.
        // We display magnitude only; direction is carried by `state`.
        let power_w: f64 = battery.energy_rate().get::<watt>().abs().into();
        let current_a = if voltage_v > 0.01 {
            power_w / voltage_v
        } else {
            0.0
        };
        let design_capacity_mwh =
            (battery.energy_full_design().get::<watt_hour>() as f64 * 1000.0) as u32;
        let full_charge_capacity_mwh =
            (battery.energy_full().get::<watt_hour>() as f64 * 1000.0) as u32;

        Some(BatteryReading {
            state,
            percentage,
            power_watts: power_w,
            design_capacity_mwh,
            full_charge_capacity_mwh,
            voltage_v,
            current_a,
        })
    }
}

impl Default for BatteryMonitor {
    fn default() -> Self {
        Self::new()
    }
}
