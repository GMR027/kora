use std::path::Path;

use ab_glyph::{FontArc, PxScale};
use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut, text_size};
use imageproc::rect::Rect;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

const PALETTE: &[(u8, u8, u8)] = &[
    (43, 74, 94),
    (92, 58, 92),
    (122, 64, 54),
    (42, 86, 74),
    (58, 62, 110),
    (110, 84, 42),
];

/// Portada simple y determinista: color derivado del título y texto centrado.
pub fn render_cover(title: &str, subtitle: &str, path: &Path) -> Result<()> {
    let bytes = render_cover_bytes(title, subtitle)?;
    std::fs::write(path, bytes)
        .with_context(|| format!("no se pudo guardar la portada {}", path.display()))?;
    Ok(())
}

/// Devuelve la portada generada como PNG en memoria para guardarla como asset.
pub fn render_cover_bytes(title: &str, subtitle: &str) -> Result<Vec<u8>> {
    let image = build_cover(title, subtitle)?;
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .context("no se pudo codificar la portada")?;
    Ok(bytes)
}

fn build_cover(title: &str, subtitle: &str) -> Result<RgbaImage> {
    let font = FontArc::try_from_slice(FONT_BYTES).context("fuente de portada inválida")?;
    let (base, accent) = colors(title);
    let width = 1200u32;
    let height = 1600u32;
    let mut image = RgbaImage::new(width, height);

    for y in 0..height {
        let factor = y as f32 / height as f32;
        let color = blend(base, (18, 20, 24), factor * 0.45);
        for x in 0..width {
            image.put_pixel(x, y, Rgba([color.0, color.1, color.2, 255]));
        }
    }

    draw_filled_rect_mut(
        &mut image,
        Rect::at(90, 90).of_size(width - 180, 10),
        Rgba([accent.0, accent.1, accent.2, 255]),
    );

    let title = title.trim();
    let scale_value = if title.chars().count() > 48 {
        74.0
    } else {
        96.0
    };
    let scale = PxScale::from(scale_value);
    let lines = wrap(title, &font, scale, width as i32 - 240);
    let line_height = (scale_value * 1.25) as i32;
    let block_height = lines.len() as i32 * line_height;
    let mut y = (height as i32 / 2 - block_height / 2).max(220);
    for line in &lines {
        let (line_width, _) = text_size(scale, &font, line);
        let x = ((width as i32 - line_width as i32) / 2).max(120);
        draw_text_mut(
            &mut image,
            Rgba([245, 244, 240, 255]),
            x,
            y,
            scale,
            &font,
            line,
        );
        y += line_height;
    }

    let subtitle_scale = PxScale::from(42.0);
    let (subtitle_width, _) = text_size(subtitle_scale, &font, subtitle);
    let subtitle_x = ((width as i32 - subtitle_width as i32) / 2).max(90);
    draw_text_mut(
        &mut image,
        Rgba([210, 210, 205, 255]),
        subtitle_x,
        height as i32 - 220,
        subtitle_scale,
        &font,
        subtitle,
    );

    Ok(image)
}

fn colors(title: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    let seed = title.bytes().fold(17u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    });
    let base = PALETTE[seed as usize % PALETTE.len()];
    let accent = (
        base.0.saturating_add(70),
        base.1.saturating_add(60),
        base.2.saturating_add(40),
    );
    (base, accent)
}

fn blend(from: (u8, u8, u8), to: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    let mix =
        |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * factor.clamp(0.0, 1.0)).round() as u8;
    (mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}

fn wrap(text: &str, font: &FontArc, scale: PxScale, max_width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        let (width, _) = text_size(scale, font, &candidate);
        if width as i32 > max_width && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push("Kora".to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_png_cover() {
        let workspace = tempfile::tempdir().expect("tmp");
        let path = workspace.path().join("cover.png");
        render_cover("Tejido de prueba", "Kora · 3 extractos", &path).expect("portada");
        let bytes = std::fs::read(&path).expect("leer");
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
        let image = image::open(&path).expect("abrir png").to_rgba8();
        let colors: std::collections::HashSet<[u8; 4]> =
            image.pixels().map(|pixel| pixel.0).collect();
        assert!(colors.len() > 50, "la portada parece vacía");
    }
}
