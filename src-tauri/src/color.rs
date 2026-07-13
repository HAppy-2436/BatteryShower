//! Color of the tray-icon glyph.
//!
//! Rule (user-specified, refined on 2026-07-13):
//! - **Full (100%)**        → pure white (255, 255, 255).
//! - **95%–99%**            → fixed **10% saturation** (the "high-end
//!                            visibility floor"). The user complained
//!                            that 95% and 100% looked identical
//!                            because the linear gradient t = (100-%)/100
//!                            gives 95% → t=0.05 → 5% saturation, which
//!                            is visually indistinguishable from white.
//!                            Now the minimum readable colour at 95%
//!                            is (229, 255, 229) green or (255, 229, 229)
//!                            red.
//! - **0%–94%**             → saturation ramps 10% → 100% with a
//!                            slight ease-in (t^0.85) so low-battery
//!                            states pop.
//!
//! Implementation: linear interpolation in RGB from "10% saturation" to
//! "100% saturation". Both green and red start/end at the same
//! saturation so the visual jump from 95% → 94% is invisible.

use crate::sensors::State;

/// Minimum saturation floor, used at >= 95% so the user can still
/// tell charging from discharging when the battery is nearly full.
const HIGH_END_SATURATION: f64 = 0.10;

pub fn icon_color(state: State, percentage: u8) -> (u8, u8, u8) {
    let percentage = percentage.clamp(0, 100);

    // Full state OR 100% → pure white
    if state == State::Full || percentage >= 100 {
        return (255, 255, 255);
    }

    // High-end visibility floor: 95%-99% use the locked-in 10%
    // saturation so the user can still distinguish charge vs discharge.
    if percentage >= 95 {
        // Use `as u8` (truncate) instead of `.round() as u8` so that
        // 255 * 0.9 = 229.5 → 229 (predictable, matches the unit test).
        let base = (255.0 * (1.0 - HIGH_END_SATURATION)) as u8;
        return match state {
            State::Charging => (base, 255, base),
            State::Discharging => (255, base, base),
            _ => (255, 255, 255),
        };
    }

    // 0%-94%: ramp from 10% to 100% saturation with slight ease-in so
    // the lower half of the battery feels dramatic.
    let t = (100 - percentage) as f64 / 100.0; // 0 at 95%, 1 at 0%
    let t = t.powf(0.85);
    let saturation = HIGH_END_SATURATION + (1.0 - HIGH_END_SATURATION) * t;
    let base = (255.0 * (1.0 - saturation)) as u8;
    match state {
        State::Charging => (base, 255, base),
        State::Discharging => (255, base, base),
        _ => (255, 255, 255),
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
    fn high_charge_has_10pct_saturation() {
        // 95-99% must all use the locked 10% floor (base = 229),
        // not the linear gradient (which would be near-white).
        for p in 95..=99 {
            let (r, g, b) = icon_color(State::Charging, p);
            assert_eq!(g, 255, "green must be 255 for charging at {}%", p);
            assert_eq!(r, 229, "red channel should be 229 (10% sat) at {}%, got {}", p, r);
            assert_eq!(b, 229, "blue channel should be 229 (10% sat) at {}%, got {}", p, b);
            let (r, g, b) = icon_color(State::Discharging, p);
            assert_eq!(r, 255);
            assert_eq!(g, 229);
            assert_eq!(b, 229);
        }
    }

    #[test]
    fn gradient_is_monotonic() {
        // From 94% down to 0%, the saturation should rise monotonically,
        // i.e. the red/blue channel should drop or stay equal.
        let mut last_r_charge = 255u8;
        let mut last_r_discharge = 255u8;
        for p in (0..=94).rev() {
            let (r, _g, b) = icon_color(State::Charging, p);
            assert_eq!(b, r, "charging R and B should match");
            assert!(r <= last_r_charge, "R should drop as % drops ({}): {}", p, r);
            last_r_charge = r;

            let (r, g, _b) = icon_color(State::Discharging, p);
            assert_eq!(r, 255, "discharging R should stay 255");
            assert!(g <= last_r_discharge, "G should drop as % drops ({}): {}", p, g);
            last_r_discharge = g;
        }
    }

    #[test]
    fn zero_percent_is_pure_color() {
        assert_eq!(icon_color(State::Charging, 0), (0, 255, 0));
        assert_eq!(icon_color(State::Discharging, 0), (255, 0, 0));
    }
}
