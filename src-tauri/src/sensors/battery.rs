//! Battery sensor — reads charge/discharge power, capacity, percentage
//! via Windows IOCTL on \\.\BATTERY0.
//!
//! Two IOCTLs are used:
//! - `IOCTL_BATTERY_QUERY_INFORMATION` → `BATTERY_INFORMATION` (design + full-charge capacity)
//! - `IOCTL_BATTERY_QUERY_STATUS`      → `BATTERY_STATUS`      (live voltage, current, %)

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

const IOCTL_BATTERY_QUERY_INFORMATION: u32 = 0x0029_0410;
const IOCTL_BATTERY_QUERY_STATUS: u32 = 0x0029_0414;
const BATTERY_INFORMATION_LEVEL: u32 = 1;

const BATTERY_CHARGING: u32 = 0x0000_0004;
const BATTERY_DISCHARGING: u32 = 0x0000_0002;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct BatteryStatusRaw {
    power_state: u32,
    capacity: u32,
    voltage: u32,
    rate: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct BatteryInformationRaw {
    capabilities: u32,
    technology: u8,
    reserved: [u8; 3],
    chemistry: [u16; 4],
    design_capacity: u32,
    full_charged_capacity: u32,
    default_alert1: u32,
    design_cycle_count: u32,
    default_alert2: u32,
    critical_bias: u32,
}

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
    /// Charge / discharge / full-state instantaneous power, in watts (always >= 0).
    pub power_watts: f64,
    /// Reported design capacity (mWh). 0 if unknown.
    pub design_capacity_mwh: u32,
    /// Reported full-charge capacity (mWh). 0 if unknown.
    pub full_charge_capacity_mwh: u32,
    pub voltage_v: f64,
    pub current_a: f64,
}

pub struct BatteryMonitor {
    design_capacity: AtomicU32,
    full_charge_capacity: AtomicU32,
    initialized: AtomicBool,
}

impl BatteryMonitor {
    pub fn new() -> Self {
        Self {
            design_capacity: AtomicU32::new(0),
            full_charge_capacity: AtomicU32::new(0),
            initialized: AtomicBool::new(false),
        }
    }

    pub fn read(&self) -> Option<BatteryReading> {
        unsafe { self.read_inner() }
    }

    unsafe fn read_inner(&self) -> Option<BatteryReading> {
        // Open \\.\BATTERY0
        let path: Vec<u16> = "\\\\.\\BATTERY0\0".encode_utf16().collect();
        let handle: HANDLE = CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )
        .ok()?;

        // Lazily read BATTERY_INFORMATION once
        if !self.initialized.load(Ordering::Relaxed) {
            let mut info = BatteryInformationRaw::default();
            let mut bytes: u32 = 0;
            let _ = DeviceIoControl(
                handle,
                IOCTL_BATTERY_QUERY_INFORMATION,
                Some(&BATTERY_INFORMATION_LEVEL as *const _ as *const _),
                std::mem::size_of_val(&BATTERY_INFORMATION_LEVEL) as u32,
                Some(&mut info as *mut _ as *mut _),
                std::mem::size_of::<BatteryInformationRaw>() as u32,
                Some(&mut bytes),
                None,
            );
            self.design_capacity
                .store(info.design_capacity, Ordering::Relaxed);
            self.full_charge_capacity
                .store(info.full_charged_capacity, Ordering::Relaxed);
            self.initialized.store(true, Ordering::Relaxed);
        }

        // Read BATTERY_STATUS
        let mut status = BatteryStatusRaw::default();
        let mut bytes: u32 = 0;
        let ok = DeviceIoControl(
            handle,
            IOCTL_BATTERY_QUERY_STATUS,
            None,
            0,
            Some(&mut status as *mut _ as *mut _),
            std::mem::size_of::<BatteryStatusRaw>() as u32,
            Some(&mut bytes),
            None,
        );
        let _ = CloseHandle(handle);
        ok.ok()?;

        // Interpret state
        let state = if (status.power_state & BATTERY_CHARGING) != 0 {
            State::Charging
        } else if (status.power_state & BATTERY_DISCHARGING) != 0 {
            State::Discharging
        } else {
            State::Full
        };

        // Voltage is always mV
        let voltage_v = status.voltage as f64 / 1000.0;
        // Rate: positive = discharge, negative = charge. Units depend on flags,
        // but on virtually all laptops this is mW (signed). Take absolute for display.
        let power_w = status.rate.unsigned_abs() as f64 / 1000.0;
        // P = V * I  →  I = P / V
        let current_a = if voltage_v > 0.01 {
            power_w / voltage_v
        } else {
            0.0
        };

        let percentage = (status.capacity as u32).min(100) as u8;

        Some(BatteryReading {
            state,
            percentage,
            power_watts: power_w,
            design_capacity_mwh: self.design_capacity.load(Ordering::Relaxed),
            full_charge_capacity_mwh: self.full_charge_capacity.load(Ordering::Relaxed),
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
