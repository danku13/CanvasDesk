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

use crate::geometry::{UiRect, UiVec2};
use crate::layout::{grid_cells_with, LayoutBackend};
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

    /// FR-068 W1 (T2-триггер ADR-0013/ADR-0014; FR-061 D-3/D-5): те же
    /// колоночные ячейки табличной строки, что [`RowGuides::with_right_edge`],
    /// но раскладка — через ЯВНО выбранный [`LayoutBackend`] (pilot-поверхности
    /// передают `pilot_backend()` — Grid с неравными явными треками, территория
    /// taffy по T2; остальные — NativeBackend).
    ///
    /// Треки `[leader, value_w, unit_w, badge_w]` (слева направо), зазор
    /// `gap` — только по горизонтали; слот — от нуля шириной `right_edge`
    /// (ячейки — в координатах тела: тело начинается с x = 0, как и
    /// направляющие [`RowGuides::with_right_edge`]). Лидер заполняет остаток
    /// (1fr-семантика этапа D-5): после трёх фиксированных треков И ТРЁХ
    /// межтрековых зазоров (лидер|значение|юнит|бейдж) — именно при таком
    /// лидере выполняются эквивалентности с [`RowGuides::with_right_edge`]:
    /// `value_x == cells[1].x`, `unit_x == cells[2].x`, `right_edge ==
    /// cells[3].right()` (порядок вычитаний в лидере — тот же, что в
    /// `with_right_edge`: бит-эквивалентность на целых входах; на дробных —
    /// в пределах 1e-3 из-за арифметического порядка операций). Если тело
    /// уже суммы треков — лидер клампится к 0, фиксированные треки выходят
    /// за `right_edge` (политика Fit — срез не маскируется, G4).
    ///
    /// `None` — деградация «таблицы нет» (аналогично [`RowGuides::measure`]
    /// на пустом списке строк): `right_edge <= 0` либо все ширины нулевые
    /// (раскладывать нечего — направляющие не строятся).
    pub fn cells_with(
        &self,
        backend: &dyn LayoutBackend,
        right_edge: f32,
        gap: f32,
        row_h: f32,
    ) -> Option<Vec<UiRect>> {
        if right_edge <= 0.0 {
            return None;
        }
        if self.value_w == 0.0 && self.unit_w == 0.0 && self.badge_w == 0.0 {
            return None;
        }
        // Порядок вычитаний — дословно with_right_edge (unit_x → value_x):
        // лидер = остаток до левого края ячейки значения МИНУС зазор
        // лидер|значение (три зазора на четыре трека).
        let unit_x = right_edge - self.badge_w - gap - self.unit_w;
        let value_x = unit_x - gap - self.value_w;
        let leader = (value_x - gap).max(0.0);
        Some(grid_cells_with(
            backend,
            UiRect::new(0.0, 0.0, right_edge, row_h),
            &[leader, self.value_w, self.unit_w, self.badge_w],
            1,
            row_h,
            UiVec2::new(gap, 0.0),
        ))
    }
}

/// Замер естественных ширин ячеек строки (проход A): значение/юнит —
/// [`TextMeasurer::width_of_weighted`] (реальный шейпинг cosmic-text, кэш по
/// (текст, семейство, кегль, вес); mono-базис CR-009 — семейство И ВЕС
/// передаёт потребитель: вес обязан совпадать с шейпингом ячейки —
/// cosmic-text ищет лицо семейства только среди лиц точного веса, замер
/// не тем весом даёт метрики чужого системного шрифта — приёмка T9
/// FR-061), пустой юнит → 0; `badge_w` проходит насквозь. Потребители —
/// сборка строк ноды (этап B) и kit-Row (этап E, D-15).
#[allow(clippy::too_many_arguments)] // плоский контракт замера ячеек (frozen FR-061 этап A + вес)
pub fn measure_row_cells(
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    value: &str,
    unit: &str,
    badge_w: f32,
    family: &str,
    size: f32,
    weight: cosmic_text::Weight,
) -> RowCellWidths {
    RowCellWidths {
        value_w: measurer.width_of_weighted(fs, value, family, size, weight),
        unit_w: if unit.is_empty() {
            0.0
        } else {
            measurer.width_of_weighted(fs, unit, family, size, weight)
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
        let short = measure_row_cells(
            &mut m,
            &mut fs,
            "800",
            "rps",
            14.0,
            FAMILY,
            SIZE,
            cosmic_text::Weight::MEDIUM,
        );
        let long = measure_row_cells(
            &mut m,
            &mut fs,
            "1389",
            "ms·req/s",
            14.0,
            FAMILY,
            SIZE,
            cosmic_text::Weight::MEDIUM,
        );
        assert!(long.value_w > short.value_w, "«1389» шире «800»");
        assert!(long.unit_w > short.unit_w, "«ms·req/s» шире «rps»");
        assert_eq!(short.badge_w, 14.0, "бейдж проходит насквозь");
        // Скалярная строка — юнит пуст → нулевая ширина ячейки
        let scalar = measure_row_cells(
            &mut m,
            &mut fs,
            "20",
            "",
            0.0,
            FAMILY,
            SIZE,
            cosmic_text::Weight::MEDIUM,
        );
        assert_eq!(scalar.unit_w, 0.0);
        // Пустое значение (unmapped рисуется «—» потребителем) — 0
        let empty = measure_row_cells(
            &mut m,
            &mut fs,
            "",
            "",
            0.0,
            FAMILY,
            SIZE,
            cosmic_text::Weight::MEDIUM,
        );
        assert_eq!(empty.value_w, 0.0);
        // Детерминизм кэша: повтор — та же ширина
        let again = measure_row_cells(
            &mut m,
            &mut fs,
            "800",
            "rps",
            14.0,
            FAMILY,
            SIZE,
            cosmic_text::Weight::MEDIUM,
        );
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
            .map(|(v, u)| {
                measure_row_cells(
                    &mut m,
                    &mut fs,
                    v,
                    u,
                    0.0,
                    FAMILY,
                    SIZE,
                    cosmic_text::Weight::MEDIUM,
                )
            })
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

    /// FR-068 W1 (T2): деградация «таблицы нет» у `cells_with` —
    /// right_edge ≤ 0 или все ширины нулевые (аналог `measure(&[])`).
    #[test]
    fn cells_with_none_on_degenerate_inputs() {
        use crate::layout::NativeBackend;
        let g = RowGuides {
            value_w: 40.0,
            unit_w: 20.0,
            badge_w: 10.0,
            value_x: 0.0,
            unit_x: 0.0,
        };
        assert!(g.cells_with(&NativeBackend, 0.0, 6.0, 24.0).is_none());
        assert!(g.cells_with(&NativeBackend, -10.0, 6.0, 24.0).is_none());
        let empty = RowGuides {
            value_w: 0.0,
            unit_w: 0.0,
            badge_w: 0.0,
            value_x: 0.0,
            unit_x: 0.0,
        };
        assert!(empty.cells_with(&NativeBackend, 300.0, 6.0, 24.0).is_none());
    }

    /// FR-068 W1 (T2): `cells_with` на NativeBackend ≡ `with_right_edge` —
    /// x-позиции ячеек значения/юнита и правый край бейджа. На целых входах
    /// — ПОБИТОВО (одинаковый порядок вычитаний/сложений, все операции
    /// точны); на «реалистично-дробных» (value_w = 44.3) — допуск 1e-3:
    /// лидер считается вычитаниями от right_edge, ячейки grid —
    /// накоплением сложений — порядок операций разный, у f32 возможны
    /// расхождения в младших битах (ulp ≈ 3e-5 на этих величинах ≪ 1e-3).
    #[test]
    fn cells_with_native_matches_right_edge_guides() {
        use crate::layout::NativeBackend;
        // Целые входы (порядок и числа — как в with_right_edge_layouts_cells)
        let g = RowGuides {
            value_w: 40.0,
            unit_w: 20.0,
            badge_w: 10.0,
            value_x: 0.0,
            unit_x: 0.0,
        }
        .with_right_edge(300.0, 6.0);
        let cells = RowGuides {
            value_w: 40.0,
            unit_w: 20.0,
            badge_w: 10.0,
            value_x: 0.0,
            unit_x: 0.0,
        }
        .cells_with(&NativeBackend, 300.0, 6.0, 24.0)
        .unwrap();
        assert_eq!(cells.len(), 4, "4 трека: лидер/значение/юнит/бейдж");
        assert_eq!(cells[1].x, g.value_x, "value_x == cell1.x (побитово)");
        assert_eq!(cells[2].x, g.unit_x, "unit_x == cell2.x (побитово)");
        assert_eq!(cells[3].right(), 300.0, "right-edge == cell3.right()");
        assert_eq!(cells[1].w, 40.0, "ширина трека = ширина ячейки значения");
        assert_eq!(cells[2].w, 20.0, "ширина трека = ширина ячейки юнита");
        assert_eq!(cells[3].w, 10.0, "ширина трека = ширина бейджа");
        assert_eq!(cells[1].h, 24.0, "row_h проходит насквозь");
        // «Реалистично-дробные» входы — допуск 1e-3 (см. доку теста)
        let gf = RowGuides {
            value_w: 44.3,
            unit_w: 10.7,
            badge_w: 56.2,
            value_x: 0.0,
            unit_x: 0.0,
        }
        .with_right_edge(280.6, 4.5);
        let cf = RowGuides {
            value_w: 44.3,
            unit_w: 10.7,
            badge_w: 56.2,
            value_x: 0.0,
            unit_x: 0.0,
        }
        .cells_with(&NativeBackend, 280.6, 4.5, 24.0)
        .unwrap();
        assert!(
            (cf[1].x - gf.value_x).abs() <= 1e-3,
            "value_x: {} vs {}",
            cf[1].x,
            gf.value_x
        );
        assert!(
            (cf[2].x - gf.unit_x).abs() <= 1e-3,
            "unit_x: {} vs {}",
            cf[2].x,
            gf.unit_x
        );
        assert!(
            (cf[3].right() - 280.6).abs() <= 1e-3,
            "right-edge: {} vs 280.6",
            cf[3].right()
        );
    }

    /// FR-068 W1 (T2): taffy-путь Grid-раскладки ≡ native-пути. Целые
    /// Points-треки — ПОБИТОВО; дробные — допуск 1.0 ui px с фиксацией:
    /// taffy 0.14 выполняет round_layout ВСЕГДА (не фича — шаг
    /// compute_layout без гейта): все координаты/размеры округляются к
    /// целым ui px (CSS spec rounding — то же документированное
    /// расхождение W1, что у grow-долей; потому допуск 1e-3 на дробных
    /// треках недостижим — размер ячейки = разность двух округлённых
    /// позиций, ошибка до 1.0 ui px).
    #[cfg(feature = "taffy")]
    #[test]
    fn cells_with_taffy_matches_native() {
        use crate::layout::{NativeBackend, TaffyBackend};
        // Целые входы — побитово
        let gi = RowGuides {
            value_w: 40.0,
            unit_w: 20.0,
            badge_w: 10.0,
            value_x: 0.0,
            unit_x: 0.0,
        };
        let native = gi.cells_with(&NativeBackend, 300.0, 6.0, 24.0).unwrap();
        let taffy = gi.cells_with(&TaffyBackend, 300.0, 6.0, 24.0).unwrap();
        assert_eq!(native.len(), taffy.len());
        for (i, (n, t)) in native.iter().zip(taffy.iter()).enumerate() {
            assert_eq!(n, t, "целые треки: ячейка {i} побитово");
        }
        // Дробные входы — допуск 1.0 ui px (round_layout taffy, см. доку)
        let gf = RowGuides {
            value_w: 44.3,
            unit_w: 10.7,
            badge_w: 56.2,
            value_x: 0.0,
            unit_x: 0.0,
        };
        let native = gf.cells_with(&NativeBackend, 280.6, 4.5, 24.0).unwrap();
        let taffy = gf.cells_with(&TaffyBackend, 280.6, 4.5, 24.0).unwrap();
        assert_eq!(native.len(), taffy.len());
        for (i, (n, t)) in native.iter().zip(taffy.iter()).enumerate() {
            assert!(
                (n.x - t.x).abs() <= 1.0 && (n.w - t.w).abs() <= 1.0,
                "дробные треки: ячейка {i} разошлась сильнее допуска \
                 (native x={} w={}, taffy x={} w={})",
                n.x,
                n.w,
                t.x,
                t.w
            );
        }
    }
}
