//! Color of the tray-icon glyph.
//!
//! Rule (user-specified):
//! - **Full (100%)**        → pure white (255, 255, 255)
//! - **Charging**           → green, saturation rises as % drops:
//!                          99% ≈ near-white tint, 50% = mid green, 0% = pure green
//! - **Discharging**        → red, same gradient as charging
//!
//! Implementation: linear interpolation in RGB from white to the target color.

use crate::sensors::State;

pub fn icon_color(state: State, percentage: u8) -> (u8, u8, u8) {
    // Full state OR 100% → pure white
    if state == State::Full || percentage >= 100 {
        return (255, 255, 255);
    }

    // t: 0.0 at 100% (white), 1.0 at 0% (target color)
    let t = 1.0_f64 - (percentage.clamp(0, 100) as f64 / 100.0);

    // ease-in slightly: t' = t^0.85 (more dramatic as percentage drops)
    let t = t.powf(0.85);

    match state {
        State::Charging => {
            // White (255,255,255) → pure green (0,255,0)
            let rb = 255.0 - 255.0 * t;
            (rb.round() as u8, 255, rb.round() as u8)
        }
        State::Discharging => {
            // White (255,255,255) → pure red (255,0,0)
            let gb = 255.0 - 255.0 * t;
            (255, gb.round() as u8, gb.round() as u8)
        }
        State::Full => (255, 255, 255),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_is_white() {
        assert_eq!(icon_color(State::Full, 50), (255, 255, 255));
        assert_eq!(icon_color(State::Charging, 100), (255, 255, 255));
    }

    #[test]
    fn gradient_is_monotonic() {
        // At lower percentage, the green channel should stay high (charging) but
        // the red channel should drop monotonically.
        let mut last_r = 255u8;
        for p in (0..=100).rev().step_by(10) {
            let (r, g, _) = icon_color(State::Charging, p);
            assert_eq!(g, 255, "green must always be 255 for charging");
            assert!(r <= last_r, "red must decrease as % drops");
            last_r = r;
        }
    }
}
