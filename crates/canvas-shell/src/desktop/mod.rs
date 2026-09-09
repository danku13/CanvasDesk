//! Режим десктопа (T15, SPEC §7.4): встройка канваса в WorkerW — за
//! иконками рабочего стола и перед обоями.
//!
//! Модуль разбит на чистое ядро (этот файл — кроссплатформенные типы и
//! логика, тестируется на любой ОС) и cfg(windows)-дочерние модули с
//! Win32-механикой: `hierarchy` (детект Progman/DefView/WorkerW, RECIPES
//! R1/R4), `attach` (встройка и стиль-скраббинг, R2/R3), `monitor`
//! (WinEventHook + DPI-поллинг, R6/R10). Все unsafe — только в дочерних
//! модулях, с SAFETY-комментариями (AGENTS.md).

/// Стратегия встраивания — выбор по рантайм-детекту иерархии окон, НЕ по
/// номеру сборки Windows (SPEC §7.4, таблица стратегий).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedStrategy {
    /// Win10 / Win11 ≤ 23H2: SetParent на top-level WorkerW (0x052C →
    /// Explorer порождает WorkerW позади SHELLDLL_DefView).
    Classic,
    /// Win11 24H2/25H2: WS_EX_LAYERED-ребёнок Progman, Z-order между
    /// SHELLDLL_DefView (иконки, сверху) и WorkerW (обои, снизу).
    Raised,
}

/// Чистое событие shell-монитора (модуль `monitor`) в event loop
/// приложения. Хэндлы сюда не попадают — только решения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEvent {
    /// WorkerW разрушен (WinEventHook R6 — основной канал; поллинг
    /// IsWindow — резерв). Реакция — `recovery_action`.
    WorkerWDestroyed,
    /// GetDpiForWindow изменился: после репарентинга winit-события
    /// ScaleFactorChanged НЕ приходят (RECIPES R10) → свой поллинг.
    DpiChanged { dpi: u32 },
}

/// Прямоугольник в ФИЗИЧЕСКИХ пикселях (виртуальный экран, MONITORINFO).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    /// Конструктор из left/top/right/bottom (границы как Win32-RECT:
    /// right/bottom — первый пиксель ЗА прямоугольником).
    pub fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Объединение прямоугольников мониторов (EnumDisplayMonitors → union).
/// Пустой список → None.
pub fn union_rects(rects: &[ScreenRect]) -> Option<ScreenRect> {
    // fold по внешним границам: min левого/верхнего, max правого/нижнего;
    // дегенерированные/перевёрнутые rect'ы участвуют той же арифметикой.
    let (first, rest) = rects.split_first()?;
    Some(rest.iter().fold(*first, |acc, &rect| {
        ScreenRect::from_ltrb(
            acc.left.min(rect.left),
            acc.top.min(rect.top),
            acc.right.max(rect.right),
            acc.bottom.max(rect.bottom),
        )
    }))
}

// Win32-константы стилей — локальные копии значений WinUser.h: этот чистый
// модуль не зависит от windows-crate (canvas-shell компилируется на Linux).
// cfg(windows)-модули сверяют значения с windows-crate debug_assert'ами
// (см. attach.rs / hierarchy.rs).
pub const WS_CHILDWINDOW: u32 = 0x4000_0000; // WS_CHILD: окно-ребёнок родителя
pub const WS_CLIPSIBLINGS: u32 = 0x0400_0000; // клиппинг о братьях — снять (R3)
pub const WS_EX_ACCEPTFILES: u32 = 0x0000_0010; // shell-дроп до нашей логики (R3)
pub const WS_EX_APPWINDOW: u32 = 0x0004_0000; // Alt+Tab/таскбар (R3: снять)
pub const WS_EX_WINDOWEDGE: u32 = 0x0000_0100; // окно «исчезает» из WorkerW (R3: снять)
pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000; // не активируется кликом (до первого клика)
pub const WS_EX_LAYERED: u32 = 0x0008_0000; // R2 шаг 2: ДО SetParent на Raised
pub const WS_EX_NOREDIRECTIONBITMAP: u32 = 0x0200_0000; // R1-маркер raised на Progman

/// Ожидаемые стили окна после скраббинга (R3): верифицируются ПЕРЕЧИТЫ-
/// ВАНИЕМ после репарентинга — библиотеки окон перезаписывают стили
/// асинхронно (урок tao/Seelen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StylePlan {
    pub style: u32,
    pub exstyle: u32,
}

/// Поле стиля для диагностики несоответствия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleField {
    Style,
    ExStyle,
}

/// Несоответствие фактических стилей плану (причина фолбэка, R14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleMismatch {
    pub field: StyleField,
    pub expected: u32,
    pub actual: u32,
}

/// План стиль-скраббинга перед SetParent (RECIPES R3):
/// `style |= WS_CHILDWINDOW`, `style &= !WS_CLIPSIBLINGS`,
/// `exstyle &= !(WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES)`
/// — Alt+Tab не показывает окно, дроп не перехватывается shell-слоем,
/// WINDOWEDGE не «выбрасывает» окно из WorkerW. Всегда
/// `exstyle |= WS_EX_NOACTIVATE` (TASKS T15: до первого клика).
/// Для Raised дополнительно `exstyle |= WS_EX_LAYERED` (R2 шаг 2).
/// Идемпотентен; посторонние биты не трогает.
pub fn plan_style_scrub(style: u32, exstyle: u32, raised: bool) -> StylePlan {
    // Точечные бит-операции: посторонние биты не задеты, повторное
    // применение меняет только уже выставленные биты (идемпотентность).
    let style = (style | WS_CHILDWINDOW) & !WS_CLIPSIBLINGS;
    let exstyle =
        (exstyle & !(WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES)) | WS_EX_NOACTIVATE;
    let exstyle = if raised {
        exstyle | WS_EX_LAYERED
    } else {
        exstyle
    };
    StylePlan { style, exstyle }
}

/// Верификация: фактические (перечитанные GWL_STYLE/GWL_EXSTYLE) стили
/// обязаны точно совпадать с планом. Отличие → StyleMismatch с полем.
pub fn verify_styles(style: u32, exstyle: u32, plan: &StylePlan) -> Result<(), StyleMismatch> {
    // точное сравнение обеих полей; style проверяется первым — он чаще
    // перезаписывается библиотекой (урок tao, R3)
    if style != plan.style {
        return Err(StyleMismatch {
            field: StyleField::Style,
            expected: plan.style,
            actual: style,
        });
    }
    if exstyle != plan.exstyle {
        return Err(StyleMismatch {
            field: StyleField::ExStyle,
            expected: plan.exstyle,
            actual: exstyle,
        });
    }
    Ok(())
}

/// Действие по разрушению WorkerW (RECIPES R2, симметрия восстановления).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Ничего (мы не встроены / фолбэк).
    None,
    /// Raised: перевыполнить только Z-order (шаги 4–5 attach), без
    /// re-parent — R2: «полный reset не нужен».
    ReZOrder,
    /// Classic: полный re-attach (SetParent на новый WorkerW).
    FullReattach,
}

/// Выбор действия по текущей стратегии. R2: raised → ReZOrder,
/// classic → FullReattach, не встроены → None.
pub fn recovery_action(attached: Option<EmbedStrategy>) -> RecoveryAction {
    match attached {
        None => RecoveryAction::None,
        Some(EmbedStrategy::Raised) => RecoveryAction::ReZOrder,
        Some(EmbedStrategy::Classic) => RecoveryAction::FullReattach,
    }
}

/// DPI (96 = 100%) → winit-scale (f64, как window.scale_factor()).
pub fn dpi_to_scale(dpi: u32) -> f64 {
    f64::from(dpi) / 96.0
}

/// Детект WorkerW после 0x052C: retry 10 × 100 мс (RECIPES R1, Seelen).
pub const DETECT_RETRIES: u32 = 10;
pub const DETECT_RETRY_DELAY_MS: u64 = 100;
/// Резервный канал watch: поллинг IsWindow(родителей) (RECIPES R6).
pub const PARENT_POLL_MS: u32 = 2000;
/// DPI-поллинг после репарентинга (RECIPES R10: 500–1000 мс).
pub const DPI_POLL_MS: u32 = 500;

#[cfg(windows)]
pub mod attach;
#[cfg(windows)]
pub mod hierarchy;
#[cfg(windows)]
pub mod monitor;
// T17: иконки/меню/выход/краш-сейф — файлы СМЕШАННЫЕ (чистые ядра +
// cfg(windows)-блоки в одном файле, рекомендация T15-A) — подключение
// БЕЗ cfg: Linux-тесты чистых частей входят в сборку.
pub mod explorer;
pub mod icons;
pub mod interop;
pub mod menu;

/// Реэкспорт HWND (координаторская интеграционная точка T15-E):
/// canvas-app не зависит от windows-crate, но конвертирует raw-window-handle
/// (`NonZeroIsize`) в HWND для вызовов attach/monitor. Win32 HWND —
/// указатель без внутренней структуры, конвертация тривиальна (идиома
/// dragdrop/com.rs).
#[cfg(windows)]
pub use windows::Win32::Foundation::HWND;

#[cfg(test)]
mod tests {
    use super::*;

    /// Эпсилон сравнения f64-шкал (таблица 96/120/144/192).
    const EPS: f64 = 1e-9;

    // ---------- union_rects ----------

    /// Пустой список → None; единственный rect → он же без изменений.
    #[test]
    fn union_rects_empty_and_single() {
        assert_eq!(union_rects(&[]), None);
        let single = ScreenRect::from_ltrb(-5, -7, 42, 9);
        assert_eq!(union_rects(&[single]), Some(single));
    }

    /// Пересекающиеся прямоугольники: внешние границы пары.
    #[test]
    fn union_rects_intersecting() {
        let a = ScreenRect::from_ltrb(0, 0, 200, 100);
        let b = ScreenRect::from_ltrb(100, 50, 300, 150);
        assert_eq!(
            union_rects(&[a, b]),
            Some(ScreenRect::from_ltrb(0, 0, 300, 150))
        );
    }

    /// Разрозненные: union — охватывающий оба прямоугольник (не сумма
    /// площадей); порядок аргументов не влияет (min/max коммутативны).
    #[test]
    fn union_rects_disjoint_span() {
        let a = ScreenRect::from_ltrb(0, 0, 100, 50);
        let b = ScreenRect::from_ltrb(500, 200, 600, 250);
        let expected = ScreenRect::from_ltrb(0, 0, 600, 250);
        assert_eq!(union_rects(&[a, b]), Some(expected));
        assert_eq!(union_rects(&[b, a]), Some(expected));
        // охват проверяется и в width/height (виртуальный экран)
        assert_eq!((expected.width(), expected.height()), (600, 250));
    }

    /// Дегенерированные rect'ы участвуют как есть (нулевая ширина
    /// left == right влияет только на top/bottom; перевёрнутый left > right
    /// — чистая min/max-арифметика, без паник и спец-обработки).
    #[test]
    fn union_rects_degenerate_participates() {
        let normal = ScreenRect::from_ltrb(0, 0, 100, 100);
        let zero_width = ScreenRect::from_ltrb(50, 0, 50, 200);
        assert_eq!(
            union_rects(&[normal, zero_width]),
            Some(ScreenRect::from_ltrb(0, 0, 100, 200))
        );
        let flipped = ScreenRect::from_ltrb(100, 0, 50, 100);
        assert_eq!(
            union_rects(&[ScreenRect::from_ltrb(0, 0, 10, 10), flipped]),
            Some(ScreenRect::from_ltrb(0, 0, 50, 100))
        );
    }

    /// 3+ мониторов — классика виртуального экрана: FHD слева от origin,
    /// основной QHD, правый приподнят; итог — охват всех трёх.
    #[test]
    fn union_rects_virtual_screen_three_monitors() {
        let monitors = [
            ScreenRect::from_ltrb(-1920, 0, 0, 1080),
            ScreenRect::from_ltrb(0, 0, 2560, 1440),
            ScreenRect::from_ltrb(2560, -500, 5120, 1000),
        ];
        let expected = ScreenRect::from_ltrb(-1920, -500, 5120, 1440);
        assert_eq!(union_rects(&monitors), Some(expected));
        assert_eq!((expected.width(), expected.height()), (7040, 1940));
    }

    // ---------- plan_style_scrub ----------

    /// WS_CHILDWINDOW устанавливается при отсутствии и сохраняется, если
    /// уже стоит; WS_CLIPSIBLINGS снимается в обоих случаях.
    #[test]
    fn plan_scrub_childwindow_set_clipsiblings_cleared() {
        let plan = plan_style_scrub(WS_CLIPSIBLINGS, 0, false);
        assert_eq!(plan.style, WS_CHILDWINDOW);
        let plan = plan_style_scrub(WS_CHILDWINDOW | WS_CLIPSIBLINGS, 0, false);
        assert_eq!(plan.style, WS_CHILDWINDOW);
    }

    /// Каждый из трёх shell-EX-битов (APPWINDOW/WINDOWEDGE/ACCEPTFILES)
    /// снимается и по отдельности, и все вместе (R3-маска).
    #[test]
    fn plan_scrub_clears_each_shell_ex_bit() {
        for bit in [WS_EX_APPWINDOW, WS_EX_WINDOWEDGE, WS_EX_ACCEPTFILES] {
            let plan = plan_style_scrub(0, bit, false);
            assert_eq!(plan.exstyle, WS_EX_NOACTIVATE, "бит {bit:#010x} не снят");
        }
        let all = WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES;
        let plan = plan_style_scrub(0, all, false);
        assert_eq!(plan.exstyle, WS_EX_NOACTIVATE);
    }

    /// Посторонние биты обоих полей сохраняются (скраббинг точечный, R3):
    /// 0x00FF_0001 — WS_POPUP-подобный набор без наших масок,
    /// 0x0002_0000 — WS_EX_TOOLWINDOW.
    #[test]
    fn plan_scrub_preserves_foreign_bits() {
        let style_in: u32 = 0x00FF_0001;
        let ex_in: u32 = 0x0002_0000 | WS_EX_APPWINDOW;
        let plan = plan_style_scrub(style_in, ex_in, false);
        assert_eq!(plan.style, style_in | WS_CHILDWINDOW);
        assert_eq!(plan.exstyle, 0x0002_0000 | WS_EX_NOACTIVATE);
    }

    /// Идемпотентность: применение плана к его же результату ничего не
    /// меняет (грязный вход со всеми снимаемыми битами, Raised).
    #[test]
    fn plan_scrub_idempotent() {
        let dirty_style = 0x00CF_0000 | WS_CLIPSIBLINGS; // WS_CAPTION-набор
        let dirty_ex = WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES | 0x0001_0000;
        let once = plan_style_scrub(dirty_style, dirty_ex, true);
        let twice = plan_style_scrub(once.style, once.exstyle, true);
        assert_eq!(once, twice);
        // и для Classic тоже (LAYERED не расставляется повторно)
        let once = plan_style_scrub(dirty_style, dirty_ex, false);
        let twice = plan_style_scrub(once.style, once.exstyle, false);
        assert_eq!(once, twice);
    }

    /// Нулевые входы: минимальный план — WS_CHILDWINDOW и NOACTIVATE (+
    /// LAYERED для Raised).
    #[test]
    fn plan_scrub_zero_inputs() {
        let plan = plan_style_scrub(0, 0, false);
        assert_eq!(plan.style, WS_CHILDWINDOW);
        assert_eq!(plan.exstyle, WS_EX_NOACTIVATE);
        let raised = plan_style_scrub(0, 0, true);
        assert_eq!(raised.style, WS_CHILDWINDOW);
        assert_eq!(raised.exstyle, WS_EX_NOACTIVATE | WS_EX_LAYERED);
    }

    /// NOACTIVATE ставится всегда (до первого клика, TASKS T15); LAYERED —
    /// только Raised (R2 шаг 2), Classic — без него; снятые shell-биты не
    /// возвращаются ни в одной стратегии.
    #[test]
    fn plan_scrub_strategy_ex_bits() {
        let classic = plan_style_scrub(0, 0, false);
        assert_eq!(classic.exstyle & WS_EX_NOACTIVATE, WS_EX_NOACTIVATE);
        assert_eq!(classic.exstyle & WS_EX_LAYERED, 0);
        let raised = plan_style_scrub(0, 0, true);
        assert_eq!(raised.exstyle & WS_EX_NOACTIVATE, WS_EX_NOACTIVATE);
        assert_eq!(raised.exstyle & WS_EX_LAYERED, WS_EX_LAYERED);
        let dirty = WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES;
        assert_eq!(plan_style_scrub(0, dirty, true).exstyle & dirty, 0);
    }

    // ---------- verify_styles ----------

    /// Полное совпадение обеих полей → Ok.
    #[test]
    fn verify_styles_exact_match_ok() {
        let plan = plan_style_scrub(0x00CF_0000, 0x0003_0000, true);
        assert_eq!(verify_styles(plan.style, plan.exstyle, &plan), Ok(()));
    }

    /// Расхождение style → Err с полем Style и значениями expected/actual;
    /// при расхождении обоих полей первым сообщается Style.
    #[test]
    fn verify_styles_style_mismatch_reports_field() {
        let plan = plan_style_scrub(WS_CLIPSIBLINGS, 0, false);
        // tao/winit пере-поставил CLIPSIBLINGS после репарентинга (R3)
        let actual = plan.style | WS_CLIPSIBLINGS;
        assert_eq!(
            verify_styles(actual, plan.exstyle, &plan),
            Err(StyleMismatch {
                field: StyleField::Style,
                expected: plan.style,
                actual,
            })
        );
        // оба поля разошлись — приоритет Style
        assert_eq!(
            verify_styles(actual, plan.exstyle | WS_EX_APPWINDOW, &plan),
            Err(StyleMismatch {
                field: StyleField::Style,
                expected: plan.style,
                actual,
            })
        );
    }

    /// Расхождение exstyle (style совпал) → Err с полем ExStyle.
    #[test]
    fn verify_styles_exstyle_mismatch_reports_field() {
        let plan = plan_style_scrub(0, WS_EX_APPWINDOW, false);
        // APPWINDOW вернулся — окно снова в Alt+Tab, фолбэк (R14)
        let actual = plan.exstyle | WS_EX_APPWINDOW;
        assert_eq!(
            verify_styles(plan.style, actual, &plan),
            Err(StyleMismatch {
                field: StyleField::ExStyle,
                expected: plan.exstyle,
                actual,
            })
        );
    }

    // ---------- recovery_action ----------

    /// R2: не встроены → None; Raised → только Z-order (шаги 4–5);
    /// Classic → полный re-attach.
    #[test]
    fn recovery_action_branches() {
        assert_eq!(recovery_action(None), RecoveryAction::None);
        assert_eq!(
            recovery_action(Some(EmbedStrategy::Raised)),
            RecoveryAction::ReZOrder
        );
        assert_eq!(
            recovery_action(Some(EmbedStrategy::Classic)),
            RecoveryAction::FullReattach
        );
    }

    // ---------- dpi_to_scale ----------

    /// Таблица стандартных DPI (SPEC §6.5): 96/120/144/192 → 1.0/1.25/1.5/2.0.
    #[test]
    fn dpi_to_scale_standard_table() {
        for (dpi, scale) in [(96u32, 1.0), (120, 1.25), (144, 1.5), (192, 2.0)] {
            let got = dpi_to_scale(dpi);
            assert!((got - scale).abs() < EPS, "dpi {dpi}: {got} != {scale}");
        }
    }

    // ---------- константы ----------

    /// Точные значения WinUser.h: модуль — локальные копии без windows-crate,
    /// сверка гарантирует их совпадение с реальными Win32-константами
    /// (windows-crate сверяется debug_assert'ами в cfg(windows)-модулях).
    #[test]
    fn constants_exact_winuser_values() {
        assert_eq!(WS_CHILDWINDOW, 0x4000_0000);
        assert_eq!(WS_CLIPSIBLINGS, 0x0400_0000);
        assert_eq!(WS_EX_ACCEPTFILES, 0x0000_0010);
        assert_eq!(WS_EX_APPWINDOW, 0x0004_0000);
        assert_eq!(WS_EX_WINDOWEDGE, 0x0000_0100);
        assert_eq!(WS_EX_NOACTIVATE, 0x0800_0000);
        assert_eq!(WS_EX_LAYERED, 0x0008_0000);
        assert_eq!(WS_EX_NOREDIRECTIONBITMAP, 0x0200_0000);
    }

    /// Битовые инварианты: WS_CHILDWINDOW не пересекается с WS_CLIPSIBLINGS;
    /// весь WS_EX_*-набор попарно дизъюнктен и не задевает оба WS-флага —
    /// маски scrub'а меняют ровно заявленные биты.
    #[test]
    fn constants_bit_disjointness() {
        let ex_bits = [
            WS_EX_ACCEPTFILES,
            WS_EX_APPWINDOW,
            WS_EX_WINDOWEDGE,
            WS_EX_NOACTIVATE,
            WS_EX_LAYERED,
            WS_EX_NOREDIRECTIONBITMAP,
        ];
        assert_eq!(WS_CHILDWINDOW & WS_CLIPSIBLINGS, 0);
        for &ex in &ex_bits {
            assert_eq!(ex & WS_CHILDWINDOW, 0, "{ex:#010x} задевает WS_CHILDWINDOW");
            assert_eq!(
                ex & WS_CLIPSIBLINGS,
                0,
                "{ex:#010x} задевает WS_CLIPSIBLINGS"
            );
        }
        // попарная дизъюнктность WS_EX_*-набора
        for (i, &a) in ex_bits.iter().enumerate() {
            for &b in &ex_bits[i + 1..] {
                assert_eq!(a & b, 0, "пересечение {a:#010x} и {b:#010x}");
            }
        }
    }
}
