//! FR-051 U1 (PRD-0009 F-2, §7.3): слоёная модель экрана — 9 фиксированных
//! полос. Draw-порядок выводится из реестра по возрастанию полосы, порядок
//! вызовов отрисовки в `RedrawRequested` перестаёт быть источником z.
//!
//! Внутри L0/L1 существующий `canvas-render/src/zorder.rs` сохраняется без
//! изменений (PRD-0009 §10 non-goals).

/// Полоса экранного слоя. Порядок вариантов = порядок отрисовки
/// (нижние раньше верхних).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UiLayer {
    /// L0 — мир: карточки, рёбра, thumbs (`zorder::plan_z_order` без изменений).
    World,
    /// L1 — мир-оверлеи: wheel-сектора, guides, what-if пульс.
    WorldOverlay,
    /// L2 — инлайн-виджеты поверх карточек (текстовые поля, порты).
    Widgets,
    /// L3 — доки/бары: what-if бар, палитра шаблонов, поиск, настройки, help.
    Panels,
    /// L4 — попапы: тултипы, dropdown-меню, hints, combobox-листы.
    Popups,
    /// L5 — модали: онбординг, галерея схем, диалоги, main stage, defense.
    Modals,
    /// L6 — drag&drop: перетаскивание шаблонов/файлов.
    Drag,
    /// L7 — тосты.
    Toasts,
    /// L8 — debug-оверлей (F9, F-10).
    Debug,
}

impl UiLayer {
    /// Все полосы в порядке отрисовки (по возрастанию).
    pub const DRAW_ORDER: [UiLayer; 9] = [
        UiLayer::World,
        UiLayer::WorldOverlay,
        UiLayer::Widgets,
        UiLayer::Panels,
        UiLayer::Popups,
        UiLayer::Modals,
        UiLayer::Drag,
        UiLayer::Toasts,
        UiLayer::Debug,
    ];

    /// Номер полосы L0..L8 (стабильный контракт для debug-оверлея и линтов).
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Машиночитаемое имя полосы (debug-оверлей F-10, подписи «слой/поверхность»).
    pub const fn label(self) -> &'static str {
        match self {
            UiLayer::World => "World",
            UiLayer::WorldOverlay => "WorldOverlay",
            UiLayer::Widgets => "Widgets",
            UiLayer::Panels => "Panels",
            UiLayer::Popups => "Popups",
            UiLayer::Modals => "Modals",
            UiLayer::Drag => "Drag",
            UiLayer::Toasts => "Toasts",
            UiLayer::Debug => "Debug",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Контракт §7.3 PRD-0009: ровно 9 полос, порядок фиксирован.
    #[test]
    fn draw_order_matches_prd_bands() {
        let expected = [
            (0, UiLayer::World),
            (1, UiLayer::WorldOverlay),
            (2, UiLayer::Widgets),
            (3, UiLayer::Panels),
            (4, UiLayer::Popups),
            (5, UiLayer::Modals),
            (6, UiLayer::Drag),
            (7, UiLayer::Toasts),
            (8, UiLayer::Debug),
        ];
        for (i, (n, layer)) in expected.iter().enumerate() {
            assert_eq!(UiLayer::DRAW_ORDER[i], *layer);
            assert_eq!(layer.as_u8(), *n);
        }
        assert_eq!(UiLayer::DRAW_ORDER.len(), 9);
    }

    #[test]
    fn partial_ord_is_draw_order() {
        assert!(UiLayer::World < UiLayer::WorldOverlay);
        assert!(UiLayer::Modals < UiLayer::Toasts);
        assert!(UiLayer::Toasts < UiLayer::Debug);
    }

    #[test]
    fn labels_are_unique() {
        let mut labels: Vec<&str> = UiLayer::DRAW_ORDER.iter().map(|l| l.label()).collect();
        let n = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), n);
    }
}
