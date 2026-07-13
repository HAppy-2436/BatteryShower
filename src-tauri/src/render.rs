//! Renders a 32×32 RGBA8 tray-icon image showing the power value (e.g. "23.4")
//! in a system monospace font, with a glyph color derived from
//! [`crate::color::icon_color`].
//!
//! The icon is **transparent** except for the glyph itself — Windows taskbar
//! background shows through, so the same icon works on light & dark themes.

use ab_glyph::{point, Font, FontRef, Glyph, PxScale, ScaleFont};
use imageproc::drawing::draw_text_mut;
use image::{ImageBuffer, Rgba};

use crate::color::icon_color;
use crate::sensors::State;

/// System monospace font shipped with every Windows install (Win2k → Win11).
/// Public-domain equivalent of a custom OFL font; guarantees consistent look.
const FONT_DATA: &[u8] = include_bytes!("../assets/Consola.ttf");

const ICON_SIZE: u32 = 32;

/// Render a 32×32 RGBA8 PNG of the power value in tray-glyph style.
pub fn render_icon(value: &str, state: State, percentage: u8) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([0, 0, 0, 0]));

    if value.is_empty() {
        return img.into_raw();
    }

    let (r, g, b) = icon_color(state, percentage);
    let text_color = Rgba([r, g, b, 255]);

    let font = FontRef::try_from_slice(FONT_DATA).expect("failed to parse embedded Consola.ttf");

    // Adaptive font size: small → larger; keep right-aligned (drawn anchored).
    let scale: PxScale = match value.chars().count() {
        1 => PxScale::from(26.0),
        2 => PxScale::from(22.0),
        3 => PxScale::from(17.0),
        4 => PxScale::from(14.0),
        _ => PxScale::from(12.0),
    };

    let scaled = font.as_scaled(scale);

    // Measure glyph widths
    let total_w: f32 = value
        .chars()
        .map(|c| {
            let glyph_id = font.glyph_id(c);
            scaled.h_advance(glyph_id)
        })
        .sum();

    // Vertical centering: use ascent/descent to get true visual height.
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let visual_h = ascent - descent;
    let baseline_y = (ICON_SIZE as f32 - visual_h) / 2.0 + ascent;

    // Horizontal centering
    let start_x = (ICON_SIZE as f32 - total_w) / 2.0;

    // Draw each glyph. `imageproc::drawing::draw_text_mut` rasterises a single
    // glyph at a time; we call it per-char to keep spacing precise.
    let mut x = start_x;
    for c in value.chars() {
        let glyph: Glyph = font.glyph_id(c).with_scale_and_position(scale, point(x, baseline_y));
        draw_text_mut(&mut img, text_color, x as i32, (baseline_y - ascent) as i32, scale, &font, &c.to_string());
        x += scaled.h_advance(font.glyph_id(c));
    }

    img.into_raw()
}

/// Default transparent placeholder used on startup.
pub fn default_icon() -> Vec<u8> {
    let mut data = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    data
}
