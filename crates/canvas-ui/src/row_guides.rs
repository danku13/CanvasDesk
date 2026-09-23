//! Колоночные направляющие табличного тела ноды (D-3 FR-061, Н-3 пре-PRD
//! PRD-0004) — проход A двухпроходной раскладки (анализ §3.2).
//!
//! Таблица ноды едина: все строки данных (авто-строки, параметры, расчёт,
//! Σ-строка) делят один набор направляющих — ячейки «значение»/«юнит»
//! фиксированной ширины = max-ширина по всем строкам ноды, текст прижат
//! вправо → цифры и доменные юниты всегда на своей вертикали (§3.1).
//!
//! Слои (архитектурное правило FR-061): домен (части значения) —
//! `canvas-core::expr` (`display_parts`), замер/геометрия — здесь
//! (`canvas-ui`, чистые функции без canvas-core-зависимости, G7),
//! исполнение (квады/тексты) — `canvas-render` (этап B, `row_grid.rs`).
//!
//! Имена: Ф-14 — слово «guides» занято snap-осями (`canvas-render/src/
//! guides.rs`), тип направляющих таблицы — [`RowGuides`], колоночные
//! направляющие.

use crate::measure::TextMeasurer;

/// Естественные ширины ячеек одной строки данных (проход A, §3.2):
/// ширина значения, ширина юнита, ширина бейджа. Замер значения/юнита —
/// [`measure_row_cells`] (TextMeasurer F-6); бейдж — известная ширина
/// пилюли/иконки, проходит насквозь.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowCellWidths {
    /// Естественная ширина текста значения (лог. px).
    pub value_w: f32,
    /// Естественная ширина текста юнита (лог. px; скаляр → 0).
    pub unit_w: f32,
    /// Ширина бейджа (пилюля каскада Р-1/what-if/unmapped; примечания
    /// полосы D в колонку не входят — анализ §3.1).
    pub badge_w: f32,
}

/// Колоночные направляющие таблицы ноды (мир-px уровня раскладки тела):
/// ширины колонок (max по всем строкам — проход A) и левые края ячеек
/// «значение»/«юнит» от правого края тела (проход B ноды, [`RowGuides::
/// with_right_edge`]). Порядок колонок справа налево: бейдж → юнит →
/// значение; лидер заполняет остаток до формулы (D-5, этап B).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowGuides {
    /// Ширина ячейки значения = max ширины значений по ноде.
    pub value_w: f32,
    /// Ширина ячейки юнита = max ширины юнитов по ноде.
    pub unit_w: f32,
    /// Резерв бейдж-колонки = max ширины бейджей по ноде.
    pub badge_w: f32,
    /// Левый край ячейки значения (текст прижат вправо — потребителю
    /// `left = value_x + value_w − text_w`, D-4).
    pub value_x: f32,
    /// Левый край ячейки юнита.
    pub unit_x: f32,
}

impl RowGuides {
    /// Проход A: направляющие ширины = max по всем строкам ноды
    /// (T3-oracle: `value_w = max(width_of(value))`). Пустой список →
    /// `None` — таблицы нет, направляющие не строятся. Детерминизм:
    /// одинаковые входы → идентичные направляющие (инвариант 2 FR-050 —
    /// повторный пересчёт не «дышит»).
    pub fn measure(rows: &[RowCellWidths]) -> Option<Self> {
        let mut value_w = 0.0_f32;
        let mut unit_w = 0.0_f32;
        let mut badge_w = 0.0_f32;
        for row in rows {
            value_w = value_w.max(row.value_w);
            unit_w = unit_w.max(row.unit_w);
            badge_w = badge_w.max(row.badge_w);
        }
        if rows.is_empty() {
            return None;
        }
        Some(Self {
            value_w,
            unit_w,
            badge_w,
            value_x: 0.0,
            unit_x: 0.0,
        })
    }

    /// Проход B (нода): проставить левые края ячеек от правого края тела
    /// (`right_edge` — правая граница контентной зоны тела с учётом
    /// `BODY_PADDING`; зазор `gap` между соседними ячейками — параметр
    /// потребителя, декларативно без магических констант). Порядок
    /// справа налево: бейдж → юнит → значение (анализ §3.1).
    pub fn with_right_edge(mut self, right_edge: f32, gap: f32) -> Self {
        self.unit_x = right_edge - self.badge_w - gap - self.unit_w;
        self.value_x = self.unit_x - gap - self.value_w;
        self
    }

    /// Правый край ячейки значения (направляющая чисел) — точка прижатия
    /// текста значения; лидер рисуется от конца формулы до этой X (D-5).
    pub fn value_right(&self) -> f32 {
        self.value_x + self.value_w
    }

    /// Правый край ячейки юнита (направляющая юнитов).
    pub fn unit_right(&self) -> f32 {
        self.unit_x + self.unit_w
    }
}

/// Замер естественных ширин ячеек строки (проход A): значение/юнит —
/// [`TextMeasurer::width_of`] (реальный шейпинг cosmic-text, кэш по
/// (текст, семейство, кегль); mono-базис CR-009 — семейство передаёт
/// потребитель), пустой юнит → 0; `badge_w` проходит насквозь. Потребители —
/// сборка строк ноды (этап B) и kit-Row (этап E, D-15).
pub fn measure_row_cells(
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    value: &str,
    unit: &str,
    badge_w: f32,
    family: &str,
    size: f32,
) -> RowCellWidths {
    RowCellWidths {
        value_w: measurer.width_of(fs, value, family, size),
        unit_w: if unit.is_empty() {
            0.0
        } else {
            measurer.width_of(fs, unit, family, size)
        },
        badge_w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминированный FontSystem: только вшитый рендером шрифт (тот же
    /// файл `NotoSansDisplay-Medium.ttf`, что FONT_DATA canvas-render) —
    /// метрики тестов = метрикам рендера и одинаковы на всех платформах CI
    /// (практика measure.rs).
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    const FAMILY: &str = "Noto Sans Display";
    const SIZE: f32 = 12.0;

    /// D-3 (FR-061, T3-oracle): направляющие ширины = max по всем строкам
    /// ноды; пустой список → None.
    #[test]
    fn measure_takes_max_across_rows() {
        let rows = [
            RowCellWidths {
                value_w: 30.0,
                unit_w: 18.0,
                badge_w: 0.0,
            },
            RowCellWidths {
                value_w: 44.0,
                unit_w: 10.0,
                badge_w: 56.0,
            },
            RowCellWidths {
                value_w: 12.0,
                unit_w: 24.0,
                badge_w: 20.0,
            },
        ];
        let g = RowGuides::measure(&rows).unwrap();
        assert_eq!(g.value_w, 44.0, "значение — max по ноде");
        assert_eq!(g.unit_w, 24.0, "юнит — max по ноде");
        assert_eq!(g.badge_w, 56.0, "бейдж — max по ноде");
        assert!(RowGuides::measure(&[]).is_none(), "нет строк — нет таблицы");
    }

    /// D-3 (FR-061): проход B — левые края ячеек от правого края тела,
    /// порядок справа налево бейдж → юнит → значение с зазором gap
    /// (анализ §3.1); направляющие правых краёв — точки прижатия.
    #[test]
    fn with_right_edge_layouts_cells_right_to_left() {
        let g = RowGuides {
            value_w: 40.0,
            unit_w: 20.0,
            badge_w: 10.0,
            value_x: 0.0,
            unit_x: 0.0,
        }
        .with_right_edge(300.0, 6.0);
        // бейдж [290..300], gap 6 → юнит [264..284], gap 6 → значение [218..258]
        assert_eq!(g.unit_x, 264.0);
        assert_eq!(g.value_x, 218.0);
        assert_eq!(g.unit_right(), 284.0, "направляющая юнитов");
        assert_eq!(g.value_right(), 258.0, "направляющая чисел");
    }

    /// D-3 (FR-061): детерминизм прохода A — одинаковые входы дают
    /// идентичные направляющие (повторный пересчёт не «дышит», инвариант 2
    /// FR-050); порядок строк не влияет (max коммутативен).
    #[test]
    fn guides_are_deterministic_and_order_free() {
        let a = RowCellWidths {
            value_w: 30.0,
            unit_w: 18.0,
            badge_w: 0.0,
        };
        let b = RowCellWidths {
            value_w: 44.0,
            unit_w: 10.0,
            badge_w: 56.0,
        };
        let g1 = RowGuides::measure(&[a, b])
            .unwrap()
            .with_right_edge(200.0, 4.0);
        let g2 = RowGuides::measure(&[b, a])
            .unwrap()
            .with_right_edge(200.0, 4.0);
        assert_eq!(g1, g2, "порядок строк не меняет направляющие");
        // Повторный замер тех же строк — идентичен
        let g3 = RowGuides::measure(&[a, b])
            .unwrap()
            .with_right_edge(200.0, 4.0);
        assert_eq!(g1, g3);
    }

    /// D-3 (FR-061): `measure_row_cells` замеряет реальным шрифтом —
    /// ширина растёт с длиной текста; пустой юнит → 0; badge проходит
    /// насквозь; результат детерминирован (кэш TextMeasurer).
    #[test]
    fn measure_row_cells_uses_real_shaping() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let short = measure_row_cells(&mut m, &mut fs, "800", "rps", 14.0, FAMILY, SIZE);
        let long = measure_row_cells(&mut m, &mut fs, "1389", "ms·req/s", 14.0, FAMILY, SIZE);
        assert!(long.value_w > short.value_w, "«1389» шире «800»");
        assert!(long.unit_w > short.unit_w, "«ms·req/s» шире «rps»");
        assert_eq!(short.badge_w, 14.0, "бейдж проходит насквозь");
        // Скалярная строка — юнит пуст → нулевая ширина ячейки
        let scalar = measure_row_cells(&mut m, &mut fs, "20", "", 0.0, FAMILY, SIZE);
        assert_eq!(scalar.unit_w, 0.0);
        // Пустое значение (unmapped рисуется «—» потребителем) — 0
        let empty = measure_row_cells(&mut m, &mut fs, "", "", 0.0, FAMILY, SIZE);
        assert_eq!(empty.value_w, 0.0);
        // Детерминизм кэша: повтор — та же ширина
        let again = measure_row_cells(&mut m, &mut fs, "800", "rps", 14.0, FAMILY, SIZE);
        assert_eq!(short, again);
    }

    /// D-3 (FR-061): сквозной oracle прохода A+B на строках с реальным
    /// замером — направляющая чисел правее направляющей юнитов, обе внутри
    /// правого края; пустые юниты не сдвигают направляющую чисел.
    #[test]
    fn guides_end_to_end_from_row_texts() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let texts: [(&str, &str); 4] =
            [("800", "rps"), ("1389", "rps"), ("50", "req/s"), ("20", "")];
        let rows: Vec<RowCellWidths> = texts
            .iter()
            .map(|(v, u)| measure_row_cells(&mut m, &mut fs, v, u, 0.0, FAMILY, SIZE))
            .collect();
        let g = RowGuides::measure(&rows)
            .unwrap()
            .with_right_edge(280.0, 5.0);
        assert!(g.value_right() < g.unit_right(), "числа левее юнитов");
        assert!(g.unit_right() <= 280.0, "бейдж-резерв 0 — юнит у края");
        // «1389» (самое широкое значение) задало направляющую
        let wide = m.width_of(&mut fs, "1389", FAMILY, SIZE);
        assert_eq!(g.value_w, wide);
    }
}
