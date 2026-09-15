//! Контрастность по WCAG 2.1 и автоматический выбор цвета текста на заливке.
//!
//! Проблема (полевые отчёты): на цветных карточках текст темы не обязан
//! читаться — светлые hex-заливки в тёмной теме получают белый текст,
//! тёмные пресеты в светлой — тёмный. Решение — тот же приём, что в
//! «auto contrast» браузерных DevTools: считать относительную люминанс
//! по WCAG и выбирать/корректировать цвет текста до целевого контраста.
//!
//! Целевые пороги: 4.5:1 — минимум WCAG AA для обычного текста (у нас
//! заголовки/тело заметок мелкие, AA обязателен); 7:1 (AAA) — цель для
//! заливок, которые мы контролируем сами (пресеты «1..6»).

use crate::Color;

/// WCAG 2.x: относительная люминанс sRGB-цвета (каналы 0..1).
/// Линеаризация по формуле стандарта, сумма с весами глазных каналов.
pub fn relative_luminance(rgb: [f32; 3]) -> f32 {
    let linear = |c: f32| -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = linear(rgb[0].clamp(0.0, 1.0));
    let g = linear(rgb[1].clamp(0.0, 1.0));
    let b = linear(rgb[2].clamp(0.0, 1.0));
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG 2.x: контраст двух sRGB-цветов (каналы 0..1), диапазон 1..=21.
pub fn contrast_ratio(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Контраст цвета текста glyphon с заливкой [f32; 4] (sRGB 0..1).
pub fn contrast_text_vs_fill(text: Color, fill: [f32; 4]) -> f32 {
    let rgb = [
        text.r() as f32 / 255.0,
        text.g() as f32 / 255.0,
        text.b() as f32 / 255.0,
    ];
    contrast_ratio(rgb, [fill[0], fill[1], fill[2]])
}

/// Линейная смесь sRGB-каналов: t=0 → fg, t=1 → to.
fn mix(fg: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    [
        fg[0] + (to[0] - fg[0]) * t,
        fg[1] + (to[1] - fg[1]) * t,
        fg[2] + (to[2] - fg[2]) * t,
    ]
}

fn to_color(rgb: [f32; 3], alpha: u8) -> Color {
    let ch = |c: f32| -> u8 { (c.clamp(0.0, 1.0) * 255.0).round() as u8 };
    Color::rgba(ch(rgb[0]), ch(rgb[1]), ch(rgb[2]), alpha)
}

/// Лучший из двух «чернил» для заливки: тот, что даёт больший контраст.
/// Гарантия WCAG: пара чистых экстремумов (чёрный/белый) даёт ≥ 4.5:1
/// с ЛЮБОЙ заливкой (худший случай — фон #777: 4.66 у чёрного).
pub fn pick_ink(fill: [f32; 4], ink_a: Color, ink_b: Color) -> Color {
    let by_contrast = contrast_text_vs_fill(ink_a, fill) >= contrast_text_vs_fill(ink_b, fill);
    if by_contrast {
        ink_a
    } else {
        ink_b
    }
}

/// Цвет текста, читаемый на заливке `bg`:
/// 1. контраст уже ≥ `target` — цвет не трогаем (тема сохраняется);
/// 2. иначе тянем цвет к экстремуму с противоположной от фона стороны
///    (на светлом фоне темним, на тёмном светlim) минимальной добавкой —
///    оттенок сохраняется, контраст достигается;
/// 3. экстремум не спасает (средне-серый фон, где ни чёрный, ни белый не
///    дают `target` с этим цветом) — лучший из двух «чернил».
///
/// «Чернила» обязаны быть контрастной парой (см. `ThemeColors::ink_candidates`).
pub fn ensure_contrast(fg: Color, bg: [f32; 4], target: f32, ink_a: Color, ink_b: Color) -> Color {
    if contrast_text_vs_fill(fg, bg) >= target {
        return fg;
    }
    let bg_rgb = [bg[0], bg[1], bg[2]];
    let fg_rgb = [
        fg.r() as f32 / 255.0,
        fg.g() as f32 / 255.0,
        fg.b() as f32 / 255.0,
    ];
    let extreme: [f32; 3] = if relative_luminance(bg_rgb) >= 0.5 {
        [0.0, 0.0, 0.0]
    } else {
        [1.0, 1.0, 1.0]
    };
    // Бинарный поиск минимальной добавки экстремума. Цель — target + 1.5:
    // поиск ровно до target даёт визуально «мутный» цвет (тёмно-серый на
    // тёмном при формальном AA) — берем запас, оттенок сохраняется.
    // Если экстремум не дотягивает до цели с запасом, но достигает target —
    // ищем на target. Запас +0.05 покрывает квантование каналов в u8.
    let comfortable = target + 0.05 + 1.5;
    let (extreme_ratio, hard_target) = (
        contrast_ratio(mix(fg_rgb, extreme, 1.0), bg_rgb),
        target + 0.05,
    );
    let goal = if extreme_ratio >= comfortable {
        comfortable
    } else if extreme_ratio >= hard_target {
        hard_target
    } else {
        // Экстремум не спасает (средне-серый фон) — шаг 3, лучшие чернила
        return pick_ink(bg, ink_a, ink_b);
    };
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if contrast_ratio(mix(fg_rgb, extreme, mid), bg_rgb) >= goal {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    to_color(mix(fg_rgb, extreme, hi), fg.a())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Color = Color::rgb(0, 0, 0);
    const WHITE: Color = Color::rgb(255, 255, 255);

    fn srgb(hex: u32) -> [f32; 3] {
        [
            ((hex >> 16) & 0xff) as f32 / 255.0,
            ((hex >> 8) & 0xff) as f32 / 255.0,
            (hex & 0xff) as f32 / 255.0,
        ]
    }

    /// Эталоны WCAG: чёрный/белый = 21:1; #767676 на белом = 4.54:1
    /// (официальный пример AA-минимума); линейная люминанс 0..1.
    #[test]
    fn wcag_reference_values() {
        assert!((relative_luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert!(relative_luminance([0.0, 0.0, 0.0]).abs() < 1e-6);
        assert!((contrast_ratio([1.0; 3], [0.0; 3]) - 21.0).abs() < 1e-4);
        // #767676 — самый тёмный серый с контрастом ≥ 4.5 к белому
        assert!((contrast_ratio(srgb(0x767676), [1.0; 3]) - 4.54).abs() < 0.02);
        // Одинаковое ≤ 0.04045 линеаризуется делением (WCAG §1.3.4)
        assert!((relative_luminance([0.04, 0.04, 0.04]) - 0.04 / 12.92).abs() < 1e-6);
    }

    /// Симметрия и диапазон контраста.
    #[test]
    fn contrast_symmetric_and_bounded() {
        let a = srgb(0x6b3d3d);
        let b = srgb(0xe6e6e6);
        let ab = contrast_ratio(a, b);
        let ba = contrast_ratio(b, a);
        assert!((ab - ba).abs() < 1e-6);
        assert!((1.0..=21.0).contains(&ab));
    }

    /// Лучшие «чернила»: на светлой заливке тёмные, на тёмной светлые,
    /// на средне-сером — тот, что даёт ≥ 4.5 (у пары чёрный/белый — всегда).
    #[test]
    fn pick_ink_extremes() {
        let light_fill = [0.92, 0.87, 0.66, 1.0]; // пастель-жёлтая карточка
        assert_eq!(pick_ink(light_fill, WHITE, BLACK), BLACK);
        let dark_fill = [0.42, 0.24, 0.24, 1.0]; // тёмный пресет «red»
        assert_eq!(pick_ink(dark_fill, WHITE, BLACK), WHITE);
        // Средне-серый #777: белый 4.48 < чёрный 4.66 → чёрный
        let grey = [0.467, 0.467, 0.467, 1.0];
        assert_eq!(pick_ink(grey, WHITE, BLACK), BLACK);
        assert!(contrast_text_vs_fill(BLACK, grey) >= 4.5);
    }

    /// Читаемый цвет: читаемый не трогается; нечитаемый на светлом фоне
    /// темнеет с сохранением оттенка (hue канала R остаётся максимумом);
    /// на тёмном — светлеет.
    #[test]
    fn ensure_contrast_preserves_readable_and_hue() {
        let pastel = [0.92, 0.87, 0.66, 1.0];
        // Тёмно-синий текст на пастели читается — не тронут
        let dark_blue = Color::rgb(0x20, 0x20, 0x24);
        assert_eq!(
            ensure_contrast(dark_blue, pastel, 4.5, WHITE, BLACK),
            dark_blue
        );
        // Белый текст на пастели → затемняется до ≥ 4.5, оттенок «красноватый»
        let fixed = ensure_contrast(WHITE, pastel, 4.5, WHITE, BLACK);
        assert!(contrast_text_vs_fill(fixed, pastel) >= 4.5);
        assert!(fixed.r() >= fixed.b(), "оттенок сохранён");
        // Светло-синий на тёмной заливке → светлеет до ≥ 4.5
        let dark_fill = [0.42, 0.24, 0.24, 1.0];
        let link = Color::rgb(0x6c, 0xb6, 0xff);
        let fixed = ensure_contrast(link, dark_fill, 4.5, WHITE, BLACK);
        assert!(contrast_text_vs_fill(fixed, dark_fill) >= 4.5);
        assert!(fixed.b() >= fixed.g(), "оттенок сохранён");
    }

    /// Средне-серый фон: смешение не спасает (и чёрный, и белый < target
    /// для исходного цвета) — гарантированный fallback на лучшие чернила.
    #[test]
    fn ensure_contrast_grey_fallback() {
        let grey = [0.183 * 0.0 + 0.467, 0.467, 0.467, 1.0]; // #777
        let muted = Color::rgb(0x6c, 0xb6, 0xff); // нечитаем ни в одну сторону до 4.5
        let fixed = ensure_contrast(muted, grey, 4.5, WHITE, BLACK);
        let best = contrast_text_vs_fill(fixed, grey);
        let max_possible =
            contrast_text_vs_fill(BLACK, grey).max(contrast_text_vs_fill(WHITE, grey));
        // Fallback — лучший достижимый контраст (чернила или смесь к экстремуму)
        assert!(
            (best - max_possible).abs() < 0.5 || best >= 4.5,
            "best={best}"
        );
    }
}
