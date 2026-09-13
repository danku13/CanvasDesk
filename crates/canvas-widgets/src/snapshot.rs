//! Снапшоты виджетов (план M5 §4.5): `CapturePreview` отдаёт PNG —
//! декодируем в RGBA для текстуры (кламп 512×512).

use crate::layout::SNAPSHOT_MAX_SIDE;

/// Ошибка обработки снапшота.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("PNG не декодируется: {0}")]
    Decode(String),
}

/// Готовый снапшот: RGBA8 + размеры (тёмная сторона — до `Renderer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetSnapshot {
    pub width: u32,
    pub height: u32,
    /// RGBA, длина = width × height × 4.
    pub rgba: Vec<u8>,
}

impl WidgetSnapshot {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
            return None;
        }
        Some(Self {
            width,
            height,
            rgba,
        })
    }
}

/// Декод PNG (байты `CapturePreview`) → RGBA с клампом по стороне.
/// Даунскейл — nearest (снапшот вспомогательный, качество не критично).
pub fn decode_png_rgba(bytes: &[u8]) -> Result<WidgetSnapshot, SnapshotError> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| SnapshotError::Decode(e.to_string()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(SnapshotError::Decode("пустое изображение".into()));
    }
    if w.max(h) <= SNAPSHOT_MAX_SIDE {
        return WidgetSnapshot::new(w, h, img.into_raw())
            .ok_or_else(|| SnapshotError::Decode("неконсистентные размеры".into()));
    }
    let scale = SNAPSHOT_MAX_SIDE as f32 / w.max(h) as f32;
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Nearest);
    WidgetSnapshot::new(nw, nh, resized.into_raw())
        .ok_or_else(|| SnapshotError::Decode("неконсистентные размеры после клампа".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    /// Синтетический PNG: градиент красного/зелёного.
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, image::Rgba([x as u8, y as u8, 0x7F, 0xFF]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png)
            .expect("PNG-encode");
        buf.into_inner()
    }

    #[test]
    fn decode_small_png_passthrough() {
        let snapshot = decode_png_rgba(&png_bytes(64, 32)).expect("декод");
        assert_eq!((snapshot.width, snapshot.height), (64, 32));
        assert_eq!(snapshot.rgba.len(), 64 * 32 * 4);
        // Левый верхний пиксель — (0, 0, 0x7F, 0xFF)
        assert_eq!(&snapshot.rgba[..4], &[0, 0, 0x7F, 0xFF]);
    }

    #[test]
    fn decode_big_png_clamped() {
        // 1024×256 → кламп: ширина 512, высота 128 (пропорции сохранены)
        let snapshot = decode_png_rgba(&png_bytes(1024, 256)).expect("декод");
        assert_eq!((snapshot.width, snapshot.height), (512, 128));
        assert!(snapshot.width <= SNAPSHOT_MAX_SIDE);
        assert!(snapshot.height <= SNAPSHOT_MAX_SIDE);
    }

    #[test]
    fn garbage_rejected_without_panic() {
        assert!(decode_png_rgba(b"not a png").is_err());
        assert!(decode_png_rgba(b"").is_err());
        assert!(
            decode_png_rgba(&png_bytes(8, 8)[..20]).is_err(),
            "обрезанный PNG"
        );
    }

    #[test]
    fn constructor_validates_consistency() {
        assert!(WidgetSnapshot::new(0, 10, vec![0; 40]).is_none());
        assert!(
            WidgetSnapshot::new(10, 10, vec![0; 39]).is_none(),
            "не хватает байта"
        );
        assert!(WidgetSnapshot::new(10, 10, vec![0; 400]).is_some());
    }
}
