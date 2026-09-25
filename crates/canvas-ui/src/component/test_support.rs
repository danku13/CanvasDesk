//! Тестовые двойки (палитры/шрифт/шрифт-система) — общие для тестов component/* (W3).

use super::*;

pub(crate) const FAMILY: &str = "Noto Sans Display";

/// Палитра-двойка: два разных значения КАЖДОГО слота — тест «стиль
/// выбирает слот, а не вычисляет цвет».
pub(crate) fn palette_a() -> KitPalette {
    KitPalette {
        panel_fill: [0.1, 0.1, 0.1, 1.0],
        panel_border: [0.2, 0.2, 0.2, 1.0],
        control_fill: [0.3, 0.3, 0.3, 1.0],
        control_border: [0.4, 0.4, 0.4, 1.0],
        control_primary: [0.5, 0.5, 0.5, 1.0],
        control_danger: [0.6, 0.6, 0.6, 1.0],
        hover_fill: [0.7, 0.7, 0.7, 1.0],
        primary_hover_fill: [0.75, 0.75, 0.75, 1.0],
        selected_fill: [0.8, 0.8, 0.8, 1.0],
        text: [0.9, 0.9, 0.9, 1.0],
        text_title: [0.91, 0.91, 0.91, 1.0],
        text_muted: [0.92, 0.92, 0.92, 1.0],
        disabled_text: [0.93, 0.93, 0.93, 1.0],
        accent: [0.94, 0.94, 0.94, 1.0],
    }
}

pub(crate) fn palette_b() -> KitPalette {
    let mut p = palette_a();
    for v in [
        &mut p.panel_fill,
        &mut p.panel_border,
        &mut p.control_fill,
        &mut p.control_border,
        &mut p.control_primary,
        &mut p.control_danger,
        &mut p.hover_fill,
        &mut p.primary_hover_fill,
        &mut p.selected_fill,
        &mut p.text,
        &mut p.text_title,
        &mut p.text_muted,
        &mut p.disabled_text,
        &mut p.accent,
    ] {
        *v = [0.05, 0.05, 0.05, 0.5];
    }
    p
}

pub(crate) fn font_system() -> cosmic_text::FontSystem {
    let mut fs = cosmic_text::FontSystem::new();
    const FONT: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansDisplay-Medium.ttf");
    fs.db_mut().load_font_data(FONT.to_vec());
    fs
}
