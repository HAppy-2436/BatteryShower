//! Renders a 64×64 RGBA8 tray-icon image showing the power value (e.g. "23.4")
//! in a system monospace font, with a glyph color derived from
//! [`crate::color::icon_color`]. The buffer is then PNG-encoded for
//! Tauri 2's `Image::from_bytes` to consume — this matches the path
//! BatteryMaster uses (topabomb/BatteryMaster `src-tauri/src/tray.rs`).
//!
//! 64×64 is the Windows 10/11 taskbar tray-icon size at @2x DPI. A 32×32
//! icon gets stretched to 64×64 by the taskbar and rendered as a
//! washed-out black square with a vertical white artifact.
//!
//! The icon is **transparent** except for the glyph itself + a 1-px
//! black outline — Windows taskbar background shows through, so the
//! same icon works on light & dark themes.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::draw_text_mut;

use crate::color::icon_color;
use crate::sensors::State;

/// System monospace font shipped with every Windows install (Win2k → Win11).
const FONT_DATA: &[u8] = include_bytes!("../assets/Consola.ttf");

const ICON_SIZE: u32 = 96;

/// Render a 64×64 PNG-encoded tray icon. The returned bytes are passed
/// to `tauri::image::Image::from_bytes`.
pub fn render_icon(value: &str, state: State, percentage: u8) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([0, 0, 0, 0]));

    if !value.is_empty() {
        let (r, g, b) = icon_color(state, percentage);
        let text_color = Rgba([r, g, b, 255]);
        // Black outline so the glyph is legible on both light and dark
        // Windows taskbar themes — critical for rule #1 (white glyph on
        // full state) which would otherwise vanish on a light taskbar.
        let stroke_color = Rgba([0, 0, 0, 255]);

        let font =
            FontRef::try_from_slice(FONT_DATA).expect("failed to parse embedded Consola.ttf");

        // Adaptive font size for a 96×96 canvas. Bigger strokes for legibility
        // on 4K / @2x DPI taskbars (which is what the user reported as
        // "数字完全看不清分辨率太低太低").
        let scale: PxScale = match value.chars().count() {
            1 => PxScale::from(80.0),
            2 => PxScale::from(64.0),
            3 => PxScale::from(48.0),
            _ => PxScale::from(40.0),
        };

        let scaled = font.as_scaled(scale);

        let total_w: f32 = value
            .chars()
            .map(|c| scaled.h_advance(font.glyph_id(c)))
            .sum();

        let ascent = scaled.ascent();
        let descent = scaled.descent();
        let visual_h = ascent - descent;
        let baseline_y = (ICON_SIZE as f32 - visual_h) / 2.0 + ascent;
        let start_x = (ICON_SIZE as f32 - total_w) / 2.0;

        // 8-direction 2-px stroke (including diagonals) for a heavier
        // outline on a 96×96 canvas — the single-pixel 4-direction
        // stroke looked thin and the digits blurred into the taskbar
        // background on high-DPI displays.
        const STROKE_OFFSETS: [(i32, i32); 8] = [
            (-2, 0), (2, 0), (0, -2), (0, 2),
            (-1, -1), (-1, 1), (1, -1), (1, 1),
        ];

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
            "[BatteryShower:render] PNG encode failed: {} — falling back to 1x1 transparent",
            e
        ));
        return Vec::new();
    }
    png_bytes
}

/// 64×64 opaque gray placeholder used on first launch before any
/// battery sample arrives. PNG-encoded so it slots into the same
/// `Image::from_bytes` path as the real icon.
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
