//! Renders a 32×32 RGBA8 tray-icon image showing the power value (e.g. "23.4")
//! in a system monospace font, with a glyph color derived from
//! [`crate::color::icon_color`].
//!
//! The icon is **transparent** except for the glyph itself — Windows taskbar
//! background shows through, so the same icon works on light & dark themes.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use image::{ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;

use crate::color::icon_color;
use crate::sensors::State;

/// System monospace font shipped with every Windows install (Win2k → Win11).
const FONT_DATA: &[u8] = include_bytes!("../assets/Consola.ttf");

const ICON_SIZE: u32 = 32;

/// Render a 32×32 RGBA8 image of the power value in tray-glyph style.
pub fn render_icon(value: &str, state: State, percentage: u8) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([0, 0, 0, 0]));

    if value.is_empty() {
        return img.into_raw();
    }

    let (r, g, b) = icon_color(state, percentage);
    let text_color = Rgba([r, g, b, 255]);
    // Black outline (Rgba([0,0,0,255])) so the glyph is legible on
    // both light and dark Windows taskbar themes — critical for rule #1
    // (white glyph on full state) which would otherwise vanish on a
    // light-themed taskbar.
    let stroke_color = Rgba([0, 0, 0, 255]);

    let font = FontRef::try_from_slice(FONT_DATA).expect("failed to parse embedded Consola.ttf");

    // Adaptive font size: 1 char → big, 4+ chars → small.
    let scale: PxScale = match value.chars().count() {
        1 => PxScale::from(26.0),
        2 => PxScale::from(22.0),
        3 => PxScale::from(17.0),
        _ => PxScale::from(14.0),
    };

    let scaled = font.as_scaled(scale);

    // Measure total horizontal advance
    let total_w: f32 = value
        .chars()
        .map(|c| scaled.h_advance(font.glyph_id(c)))
        .sum();

    // Center horizontally and vertically (account for ascent/descent).
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let visual_h = ascent - descent;
    let baseline_y = (ICON_SIZE as f32 - visual_h) / 2.0 + ascent;
    let start_x = (ICON_SIZE as f32 - total_w) / 2.0;

    // 4-direction 1-px stroke offsets for the outline pass.
    const STROKE_OFFSETS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    // Per-char rendering: stroke first, then glyph on top.
    let mut x = start_x;
    for c in value.chars() {
        let g = font.glyph_id(c).with_scale_and_position(scale, point(x, baseline_y));
        if let Some(outlined) = font.outline_glyph(g) {
            let bounds = outlined.px_bounds();
            let bx = bounds.min.x as i32;
            let by = bounds.min.y as i32;
            for (dx, dy) in STROKE_OFFSETS {
                draw_text_mut(
                    &mut img,
                    stroke_color,
                    bx + dx,
                    by + dy,
                    scale,
                    &font,
                    &c.to_string(),
                );
            }
            draw_text_mut(
                &mut img,
                text_color,
                bx,
                by,
                scale,
                &font,
                &c.to_string(),
            );
        }
        x += scaled.h_advance(font.glyph_id(c));
    }

    img.into_raw()
}

/// Default transparent placeholder used on startup.
pub fn default_icon() -> Vec<u8> {
    vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize]
}
