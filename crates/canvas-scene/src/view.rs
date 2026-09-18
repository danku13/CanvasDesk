//! View-модель кадра над моделью сцены (FR-029/FR-017, перенос из
//! canvas-render при FR-037 MW1-ребейзе): чистые данные для рендера —
//! проливания параметров и what-if представления нод. Типы не тянут
//! GPU/шрифты, поэтому живут в canvas-scene (ADR-0012: без canvas-render);
//! canvas-render реэкспортирует их (зависимость render → scene, слои:
//! core → scene → render → app).

/// FR-029 (визуализация проливания): параметр ноды, запитанный входящим
/// value-ребром с `toParam` — как показать строку-присваивание в теле
/// карточки и её бейдж. Runtime-данные приложения: пересчитываются в
/// `SceneState::recompute_flow`, НЕ сериализуются в `.canvas`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpillView {
    /// Имя параметра (имя присваивания в Numi-листе ноды).
    pub param: String,
    /// Индекс строки листа с присваиванием `param = …` (None — строки нет:
    /// подмены текста и бейджа не будет, значение только в окружении формулы).
    pub line: Option<usize>,
    /// Заголовок ноды-источника (подпись «← откуда»).
    pub from_label: String,
    /// Именованный выход истока (суффикс «· output» подписи).
    pub from_output: Option<String>,
    /// Эффективное значение для бейджа — значение ребра-источника
    /// (адресация fromLine/fromOutput/узловое), т.е. то, что реально
    /// пролито в параметр. None — источник без значения: тихая деградация
    /// до локального результата строки.
    pub value: Option<String>,
}

impl SpillView {
    /// Кортеж для `canvas_core::flow::substitute_spilled_lines`
    /// (параметр, заголовок источника, именованный выход).
    pub fn as_triple(&self) -> (&str, &str, Option<&str>) {
        (
            self.param.as_str(),
            self.from_label.as_str(),
            self.from_output.as_deref(),
        )
    }
}

/// FR-017 (CP6): what-if представление ноды кадра — виртуальный исходник
/// (подмены строк активного сценария), подсветка подменённых строк и
/// дельта-строки «было → стало (+Δ)». Runtime-данные приложения
/// (пересчёт в `SceneState::recompute_flow`), НЕ сериализуются; подмены
/// базу не мутируют (инвариант 2 FR-017).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WhatIfNode {
    /// Виртуальный исходник тела: подменённые строки заменены.
    pub text: String,
    /// Индексы подменённых строк — фон-подсветка (`BodyQuadKind::WhatIfBg`).
    pub overrides: Vec<usize>,
    /// Дельта-строки по формульным строкам: (индекс строки текста,
    /// «было → стало (+Δ)») — бейдж результата строки.
    pub line_deltas: Vec<(usize, String)>,
    /// Дельта узлового итога (футер результата шаблонной/expr-ноды).
    pub footer_delta: Option<String>,
}
