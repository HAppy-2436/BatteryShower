//! Renders a 96×96 RGBA8 tray-icon image showing the power value (e.g. "23")
//! in a system monospace font, with a glyph color derived from
//! [`crate::color::icon_color`]. The buffer is then PNG-encoded for
//! Tauri 2's `Image::from_bytes` to consume — this matches the path
//! BatteryMaster uses (topabomb/BatteryMaster `src-tauri/src/tray.rs`).
//!
//! Layout notes (audit on 2026-07-13):
//! - No outline. User reported previous 2-px stroke as "完全不需要" +
//!   it inflated total ink and pushed glyphs off-canvas at the
//!   larger sizes.
//! - Centring uses ab_glyph's `px_bounds()` (actual pixel rectangle
//!   of each glyph) rather than `ascent - descent` (em-square, not
//!   pixel). The em-square math over-estimated visual height for
//!   Consolas and pushed the bottom of two-digit numbers off the
//!   96-px canvas.
//! - Font sizes mirror BatteryMaster's 64-px pattern: 60pt for 1–2
//!   digits, 38pt for ≥3 digits, scaled 1.5× to fit a 96-px canvas
//!   (so 90pt / 57pt). 90pt Consolas is roughly 67-px tall in
//!   pixel-bounds terms, which leaves ~14 px of headroom on top
//!   and bottom — verified empirically.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::draw_text_mut;

use crate::color::icon_color;
use crate::sensors::State;

/// System monospace font shipped with every Windows install (Win2k → Win11).
const FONT_DATA: &[u8] = include_bytes!("../assets/Consola.ttf");

const ICON_SIZE: u32 = 96;

/// Render a 96×96 PNG-encoded tray icon. Returned bytes go to
/// `tauri::image::Image::from_bytes`.
pub fn render_icon(value: &str, state: State, percentage: u8) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([0, 0, 0, 0]));

    if !value.is_empty() {
        let (r, g, b) = icon_color(state, percentage);
        let text_color = Rgba([r, g, b, 255]);

        let font =
            FontRef::try_from_slice(FONT_DATA).expect("failed to parse embedded Consola.ttf");

        // Match BatteryMaster's 64-px rule (60pt for 1–2 digits, 38pt for
        // ≥3) and scale 1.5× to fit our 96-px canvas.
        let scale: PxScale = match value.chars().count() {
            1 | 2 => PxScale::from(90.0),
            _ => PxScale::from(57.0),
        };

        let scaled = font.as_scaled(scale);

        // Per-glyph horizontal advance (monospace → all equal, but
        // measured precisely anyway).
        let advances: Vec<f32> = value
            .chars()
            .map(|c| scaled.h_advance(font.glyph_id(c)))
            .collect();
        let total_w: f32 = advances.iter().sum();

        // Real pixel bounding box of the whole text run. We measure
        // each glyph at its draw position so min/max y reflects the
        // *actual* glyph pixels, not the em-square.
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut x_cursor = 0.0_f32;
        for (i, c) in value.chars().enumerate() {
            let g = font
                .glyph_id(c)
                .with_scale_and_position(scale, point(x_cursor, 0.0));
            if let Some(outlined) = font.outline_glyph(g) {
                let b = outlined.px_bounds();
                if b.min.y < min_y {
                    min_y = b.min.y;
                }
                if b.max.y > max_y {
                    max_y = b.max.y;
                }
            }
            x_cursor += advances[i];
        }
        // If every char failed to outline, fall back to em-square.
        let actual_h = if max_y > min_y {
            max_y - min_y
        } else {
            (scaled.ascent() - scaled.descent()).max(1.0)
        };

        // Centring: glyph block centred in the 96-px square. We pass
        // draw_text_mut the glyph's own top-left pixel, not the centre,
        // because imageproc's draw_text_mut takes a top-left corner.
        let start_x = (ICON_SIZE as f32 - total_w) / 2.0;
        let y_offset = (ICON_SIZE as f32 - actual_h) / 2.0 - min_y;

        let mut x = start_x;
        for (i, c) in value.chars().enumerate() {
            let g = font
                .glyph_id(c)
                .with_scale_and_position(scale, point(x, y_offset));
            if let Some(outlined) = font.outline_glyph(g) {
                let b = outlined.px_bounds();
                draw_text_mut(
                    &mut img,
                    text_color,
                    b.min.x as i32,
                    b.min.y as i32,
                    scale,
                    &font,
                    &c.to_string(),
                );
            }
            x += advances[i];
        }
    }

    // PNG-encode the RGBA buffer; Tauri 2's Image::from_bytes expects PNG.
    let mut png_bytes = Vec::new();
    if let Err(e) = PngEncoder::new(&mut png_bytes).write_image(
        &img,
        ICON_SIZE,
        ICON_SIZE,
        ExtendedColorType::Rgba8,
    ) {
        crate::log::log_write(&format!(
            "[BatteryShower:render] PNG encode failed: {} — falling back to empty PNG",
            e
        ));
        return Vec::new();
    }
    png_bytes
}

/// 96×96 opaque gray placeholder used on first launch before any
/// battery sample arrives. PNG-encoded so it slots into the same
/// `Image::from_bytes` path as the live icon.
pub fn default_icon() -> Vec<u8> {
    const N: usize = (ICON_SIZE * ICON_SIZE) as usize;
    let mut img_data = vec![0u8; N * 4];
    for i in 0..N {
        img_data[i * 4] = 96;
        img_data[i * 4 + 1] = 96;
        img_data[i * 4 + 2] = 96;
        img_data[i * 4 + 3] = 255;
    }
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(ICON_SIZE, ICON_SIZE, img_data).expect("img buffer");
    let mut png = Vec::new();
    let _ = PngEncoder::new(&mut png).write_image(
        &img,
        ICON_SIZE,
        ICON_SIZE,
        ExtendedColorType::Rgba8,
    );
    png
}
