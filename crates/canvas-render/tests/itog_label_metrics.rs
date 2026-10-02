//! CPU-замер вертикальной геометрии метки «ИТОГ» в полосе результата
//! (отчёт владельца 2026-10-03: метка клипится снизу на шаблонных нодах).
//!
//! Механика регрессии (внесена FR-075, 01bc8b38): метку сдвинули к
//! вертикальному центру полосы 32 px (v_center = (32−12)/2 = 10 world px),
//! но TextBounds.bottom остался «верх полосы + RESULT_LINE_HEIGHT (16)» —
//! значение до-FR-075, когда метка сидела у нижнего края. glyphon 0.6
//! режет растеризацию глифов по bounds: строка метки занимает [10..22],
//! базлайн прописных ≈ 19.9 — всё ниже 16 отсекалось, у «ИТОГ» срезалась
//! нижняя половина букв.
//!
//! Тест без GPU фиксирует два инварианта: (1) базлайн метки внутри полосы
//! при v-center центрировании (теперь bounds = вся полоса, паттерн блока
//! значения); (2) документирует арифметику старого клипа (16 < базлайн).

use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight, Wrap};

const BADGE_FONT_SIZE: f32 = 10.0;
const BADGE_LINE_HEIGHT: f32 = 12.0;
const RESULT_LINE_HEIGHT: f32 = 16.0;
const STRIP_H: f32 = 32.0;
const SANS_FAMILY: &str = "Noto Sans Display";

#[test]
fn itog_label_baseline_fits_strip_with_v_center() {
    let zoom = 1.0f32;
    let mut fs = cosmic_text::FontSystem::new();
    let data = std::fs::read("../../assets/fonts/NotoSansDisplay-Medium.ttf")
        .expect("шрифт Noto Sans Display Medium");
    fs.db_mut().load_font_data(data);

    // Те же параметры шейпа метки, что в text.rs (кэш ноды).
    let mut buffer = Buffer::new(
        &mut fs,
        Metrics::new(BADGE_FONT_SIZE * zoom, BADGE_LINE_HEIGHT * zoom),
    );
    buffer.set_wrap(&mut fs, Wrap::None);
    buffer.set_size(&mut fs, Some(360.0 * zoom), Some(BADGE_LINE_HEIGHT * zoom));
    buffer.set_text(
        &mut fs,
        "ИТОГ",
        Attrs::new()
            .family(Family::Name(SANS_FAMILY))
            .weight(Weight::MEDIUM),
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(&mut fs, false);

    let v_center = ((STRIP_H - BADGE_LINE_HEIGHT) / 2.0) * zoom; // 10·zoom
    let mut line_y = f32::MIN;
    for run in buffer.layout_runs() {
        eprintln!(
            "run: line_top={:.2} line_y(базлайн)={:.2} line_height={:.2}",
            run.line_top, run.line_y, run.line_height
        );
        line_y = line_y.max(run.line_y);
    }
    assert!(
        line_y.is_finite(),
        "«ИТОГ»: нет layout-run — шрифт не загрузился?"
    );

    // Абсолютная геометрия от верха полосы: top буфера = +v_center,
    // базлайн глифов = v_center + line_y (glyphon: y = line_y + top − image.top).
    let baseline_abs = v_center + line_y;
    eprintln!(
        "v_center={v_center:.2} baseline_abs={baseline_abs:.2} старый bounds.bottom={RESULT_LINE_HEIGHT:.2} полоса={STRIP_H:.2}"
    );

    // Инвариант фикса: базлайн (низ прописных, выносных нет) внутри полосы —
    // при bounds = вся полоса метка не клипится.
    assert!(
        baseline_abs <= STRIP_H,
        "базлайн «ИТОГ» ({baseline_abs:.2}) вне полосы {STRIP_H} — v-center не согласован с полосой"
    );
    // Строка метки целиком в полосе (низ line-box).
    assert!(
        v_center + BADGE_LINE_HEIGHT <= STRIP_H,
        "line-box метки вылезает из полосы: v_center+line_h > {STRIP_H}"
    );
    // Документация регрессии: прежний bounds.bottom (RESULT_LINE_HEIGHT=16,
    // до-FR-075 модель «метка у нижнего края») проходил ПОНСЕ прописных —
    // glyphon срезал нижнюю часть букв. Если типографика полосы изменится
    // так, что равенство перестанет выполняться — комментарий в text.rs
    // про «+16» больше не описывает клип, тест можно снять.
    assert!(
        RESULT_LINE_HEIGHT < baseline_abs,
        "старая граница (+16) больше не режет базлайн ({baseline_abs:.2}) — арифметика регрессии изменилась"
    );
}
