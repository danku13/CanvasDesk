//! Модель данных `.canvas` (JSON Canvas 1.0, SPEC §5.1).
//!
//! Совместимость с jsoncanvas.org: неизвестные поля нод, связей и корня
//! сохраняются в `extra` (serde flatten) и не теряются при round-trip;
//! неизвестные типы нод не ломают парсинг (`node_type` — строка).

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Сторона ноды для привязки связи (JSON Canvas: `top`/`right`/`bottom`/`left`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

/// Тип ноды, выведенный из строки `type` (JSON Canvas 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Text,
    Link,
    Group,
    /// Виджет-нода M5 (SPEC §5.1/§7.6): `canvasdesk: { widgetId, props }`,
    /// рендерится через WebView2. В отличие от `Unknown` — распознаётся
    /// приложением (LOD-менеджер, меню виджетов), но для сторонних редакторов
    /// остаётся просто нодой с неизвестным типом (round-trip без потерь).
    Widget,
    /// Неизвестный тип (например, из будущих версий spec) — нода сохраняется как есть.
    Unknown,
}

/// Последний уровень детализации ноды — расширение `previewState` (SPEC §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewState {
    Thumbnail,
    Live,
    None,
}

/// Стиль линии связи — расширение `edgeStyle` (не входит в JSON Canvas 1.0,
/// сохраняется опционально; старые файлы парсятся, поле отсутствует).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeLineStyle {
    /// Сплошная линия (дефолт при отсутствии поля).
    Solid,
    /// Штрихи (черта/пропуск).
    Dashed,
    /// Одиночные точки.
    Dotted,
}

impl EdgeLineStyle {
    /// Подпись стиля в контекстном меню связи.
    pub fn label(self) -> &'static str {
        match self {
            EdgeLineStyle::Solid => "сплошная",
            EdgeLineStyle::Dashed => "пунктир",
            EdgeLineStyle::Dotted => "точки",
        }
    }
}

/// Толщина линии связи — расширение `edgeWidth` (дефолт Medium ≈ текущие
/// 2.5 world-px, см. canvas_render::EDGE_DOT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EdgeThickness {
    /// Тонкая (1.8 world-px).
    Thin,
    /// Средняя (2.5 world-px) — дефолт.
    #[default]
    Medium,
    /// Толстая (3.5 world-px).
    Thick,
}

impl EdgeThickness {
    /// Диаметр кружка линии в world-px.
    pub fn dot(self) -> f32 {
        match self {
            EdgeThickness::Thin => 1.8,
            EdgeThickness::Medium => 2.5,
            EdgeThickness::Thick => 3.5,
        }
    }

    /// Подпись толщины в контекстном меню связи.
    pub fn label(self) -> &'static str {
        match self {
            EdgeThickness::Thin => "тонкая",
            EdgeThickness::Medium => "средняя",
            EdgeThickness::Thick => "толстая",
        }
    }

    /// Переключение по циклу (три варианта).
    pub fn next(self) -> Self {
        match self {
            EdgeThickness::Thin => EdgeThickness::Medium,
            EdgeThickness::Medium => EdgeThickness::Thick,
            EdgeThickness::Thick => EdgeThickness::Thin,
        }
    }
}

/// Расширение `canvasdesk` ноды (SPEC §5.1/§7.6): виджет-нода — `widgetId`
/// + `props`; FR-013: calc-нода (text + формула) — только `expr`.
///
/// Поля опциональны, потому что объект один на оба случая; пустой объект
/// не сериализуется (skip при is_empty в Node).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasdeskExt {
    #[serde(rename = "widgetId", default, skip_serializing_if = "Option::is_none")]
    pub widget_id: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
    /// FR-013: Numi-формула text-ноды (Numi-base — модуль `expr`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

impl CanvasdeskExt {
    /// Пустое расширение (для get_or_insert при set_expr).
    pub fn empty() -> Self {
        Self {
            widget_id: None,
            props: Map::new(),
            expr: None,
        }
    }

    /// Расширение виджет-ноды: идентификатор пакета + пустые props.
    pub fn widget(widget_id: impl Into<String>) -> Self {
        Self {
            widget_id: Some(widget_id.into()),
            props: Map::new(),
            expr: None,
        }
    }
}

/// Нода канваса. `node_type` — строкой, чтобы неизвестные типы (widget и будущие)
/// не ломали парсинг (SPEC §5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    /// Путь к файлу. Отклонение от spec: допускаются абсолютные пути Windows (SPEC §5.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Цвет по JSON Canvas spec: пресет "1".."6" или "#RRGGBB".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Расширение: файл недоступен — карточка с серой рамкой (SPEC §5.1).
    #[serde(rename = "brokenLink", skip_serializing_if = "Option::is_none")]
    pub broken_link: Option<bool>,
    /// Расширение: последний уровень детализации ноды (SPEC §5.1).
    #[serde(rename = "previewState", skip_serializing_if = "Option::is_none")]
    pub preview_state: Option<PreviewState>,
    /// Расширение: данные виджет-ноды (M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvasdesk: Option<CanvasdeskExt>,
    /// FR-011: ветвление mindmap — поддерево ноды свернуто. Расширение
    /// `.canvas` (SPEC §5.1): сериализуется только при Some(true) — чужие
    /// редакторы сохраняют поле как неизвестное (round-trip без потерь).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    /// FR-012: ЯВНЫЕ дети группы (id нод; только для kind == Group).
    /// Расширение `.canvas`: сериализуется только у групп. Membership
    /// больше не чисто геометрический: нода, случайно занесённая поверх
    /// группы, ребёнком НЕ становится — только жестом вставки (drag).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<String>>,
    /// Неизвестные поля — сохраняются при round-trip (совместимость с Obsidian).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Node {
    /// Тип ноды по строке `type`.
    pub fn kind(&self) -> NodeKind {
        match self.node_type.as_str() {
            "file" => NodeKind::File,
            "text" => NodeKind::Text,
            "link" => NodeKind::Link,
            "group" => NodeKind::Group,
            "widget" => NodeKind::Widget,
            _ => NodeKind::Unknown,
        }
    }

    /// Текстовая нода-заметка.
    pub fn text(id: impl Into<String>, text: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            id: id.into(),
            node_type: "text".to_owned(),
            file: None,
            text: Some(text.into()),
            label: None,
            color: None,
            x,
            y,
            width: 260.0,
            height: 120.0,
            broken_link: None,
            preview_state: None,
            canvasdesk: None,
            collapsed: None,
            children: None,
            extra: Map::new(),
        }
    }

    /// FR-013: Numi-формула text-ноды — `canvasdesk.expr` (расширение
    /// `.canvas`; Obsidian сохраняет неизвестное поле без потерь, SPEC
    /// §5.1). None — calc-режим выключен.
    pub fn expr(&self) -> Option<&str> {
        self.canvasdesk.as_ref()?.expr.as_deref()
    }

    /// FR-013: записать/сбросить формулу (`canvasdesk.expr`). При `None`
    /// поле удаляется; если в `canvasdesk` больше ничего нет — объект
    /// целиком (round-trip чистый: пустого расширения в JSON не будет).
    /// Данные виджета (`widgetId`/`props`) не трогаются.
    pub fn set_expr(&mut self, expr: Option<String>) {
        match expr {
            Some(formula) => {
                let ext = self.canvasdesk.get_or_insert_with(CanvasdeskExt::empty);
                ext.expr = Some(formula);
            }
            None => {
                if let Some(ext) = &mut self.canvasdesk {
                    ext.expr = None;
                    if ext.widget_id.is_none() && ext.props.is_empty() {
                        self.canvasdesk = None;
                    }
                }
            }
        }
    }

    /// Нода-группа: рамка с подписью (`label`). Дети — ЯВНЫЙ список `children`
    /// (FR-012); у групп без списка (легаси-файлы) — геометрический фолбэк
    /// `group_children` (совместимость, SPEC §5.1).
    pub fn group(id: impl Into<String>, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            node_type: "group".to_owned(),
            file: None,
            text: None,
            label: None,
            color: None,
            x,
            y,
            width,
            height,
            broken_link: None,
            preview_state: None,
            canvasdesk: None,
            collapsed: None,
            children: None,
            extra: Map::new(),
        }
    }

    /// Файловая нода.
    pub fn file(
        id: impl Into<String>,
        file: impl Into<String>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id: id.into(),
            node_type: "file".to_owned(),
            file: Some(file.into()),
            text: None,
            label: None,
            color: None,
            x,
            y,
            width,
            height,
            broken_link: None,
            preview_state: None,
            canvasdesk: None,
            collapsed: None,
            children: None,
            extra: Map::new(),
        }
    }

    /// Виджет-нода (M5, SPEC §5.1/§7.6): расширение `canvasdesk`
    /// (widgetId + props), `label` — человекочитаемое имя пакета из
    /// манифеста (заголовок карточки).
    pub fn widget(
        id: impl Into<String>,
        ext: CanvasdeskExt,
        label: impl Into<String>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id: id.into(),
            node_type: "widget".to_owned(),
            file: None,
            text: None,
            label: Some(label.into()),
            color: None,
            x,
            y,
            width,
            height,
            broken_link: None,
            preview_state: None,
            canvasdesk: Some(ext),
            collapsed: None,
            children: None,
            extra: Map::new(),
        }
    }
}

/// Связь между нодами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "fromSide", skip_serializing_if = "Option::is_none")]
    pub from_side: Option<Side>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(rename = "toSide", skip_serializing_if = "Option::is_none")]
    pub to_side: Option<Side>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Расширение: стиль линии (`edgeStyle`, см. `EdgeLineStyle`).
    #[serde(rename = "edgeStyle", skip_serializing_if = "Option::is_none")]
    pub style: Option<EdgeLineStyle>,
    /// Расширение: толщина линии (`edgeWidth`, см. `EdgeThickness`).
    #[serde(rename = "edgeWidth", skip_serializing_if = "Option::is_none")]
    pub thickness: Option<EdgeThickness>,
    /// Неизвестные поля (fromEnd/toEnd и пр.) — сохраняются при round-trip.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Edge {
    /// Новая связь между нодами по id. Стороны могут быть None — тогда
    /// выводятся из взаимного положения нод (см. `edgegeom::edge_curve`).
    pub fn new(
        id: impl Into<String>,
        from_node: impl Into<String>,
        from_side: Option<Side>,
        to_node: impl Into<String>,
        to_side: Option<Side>,
    ) -> Self {
        Self {
            id: id.into(),
            from_node: from_node.into(),
            from_side,
            to_node: to_node.into(),
            to_side,
            label: None,
            color: None,
            style: None,
            thickness: None,
            extra: Map::new(),
        }
    }
}

/// Корень `.canvas`-файла.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// Неизвестные поля верхнего уровня — сохраняются при round-trip.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Canvas {
    /// Найти ноду по id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Индекс верхней ноды под world-точкой (AABB; поздняя нода в массиве — выше по z).
    pub fn hit_test(&self, point: [f32; 2]) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .rev()
            .find(|(_, node)| {
                point[0] >= node.x
                    && point[0] <= node.x + node.width
                    && point[1] >= node.y
                    && point[1] <= node.y + node.height
            })
            .map(|(index, _)| index)
    }

    /// Добавить связь.
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Удалить связь по id. Возвращает true, если связь найдена.
    pub fn remove_edge(&mut self, id: &str) -> bool {
        let before = self.edges.len();
        self.edges.retain(|edge| edge.id != id);
        self.edges.len() != before
    }

    /// Индексы связей, инцидентных ноде (в любом направлении).
    pub fn edges_of(&self, node_id: &str) -> Vec<usize> {
        self.edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.from_node == node_id || edge.to_node == node_id)
            .map(|(index, _)| index)
            .collect()
    }

    /// Следующий свободный id вида `edge-N` (суффиксы существующих не переиспользуются).
    pub fn next_edge_id(&self) -> String {
        let max_suffix = self
            .edges
            .iter()
            .filter_map(|edge| edge.id.strip_prefix("edge-"))
            .filter_map(|suffix| suffix.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("edge-{}", max_suffix + 1)
    }

    /// Удалить ноду по индексу вместе со всеми её связями (каскад, T8).
    /// Возвращает удалённую ноду; None, если индекс вне диапазона.
    pub fn remove_node(&mut self, index: usize) -> Option<Node> {
        if index >= self.nodes.len() {
            return None;
        }
        let node = self.nodes.remove(index);
        let id = node.id.clone();
        self.edges
            .retain(|edge| edge.from_node != id && edge.to_node != id);
        Some(node)
    }

    /// Удалить несколько нод каскадно со связями (CR-001, мультивыделение).
    /// Индексы валидны ДО вызова; удаление идёт от больших индексов к
    /// меньшим (сдвиги от удалений не затрагивают ещё не удалённые),
    /// дубликаты игнорируются. Возвращает удалённые ноды в порядке
    /// возрастания индексов; пусто — невалидный набор.
    pub fn remove_nodes(&mut self, indices: &[usize]) -> Vec<Node> {
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let mut removed: Vec<Node> = Vec::with_capacity(sorted.len());
        // remove_node каскадит связи по id — id не зависят от индексов
        for index in sorted.iter().rev() {
            if let Some(node) = self.remove_node(*index) {
                removed.push(node);
            }
        }
        removed.reverse();
        removed
    }

    /// Сдвинуть группу и всех её детей на (dx, dy): каждая нода сдвигается
    /// ровно один раз (вложенные группы — как обычные ноды, рекурсии нет).
    /// Возвращает индексы сдвинутых нод (группа — первой); пусто, если
    /// индекс группы невалиден.
    pub fn translate_group(&mut self, group_index: usize, dx: f32, dy: f32) -> Vec<usize> {
        if self.nodes.get(group_index).is_none() {
            return Vec::new();
        }
        let children = group_children(self, group_index);
        for index in std::iter::once(group_index).chain(children.iter().copied()) {
            if let Some(node) = self.nodes.get_mut(index) {
                node.x += dx;
                node.y += dy;
            }
        }
        let mut moved = vec![group_index];
        moved.extend(children);
        moved
    }
}

/// Индексы детей группы (FR-012): если у группы явный список `children` —
/// только он (в порядке возрастания индексов; отсутствующие в модели id
/// пропускаются). Легаси-фолбэк (списка нет) — геометрия: ноды, чей центр
/// лежит внутри rect группы (границы включительно). Вложенные группы
/// считаются обычными нодами — рекурсии нет. Чистая функция.
pub fn group_children(canvas: &Canvas, group_index: usize) -> Vec<usize> {
    let Some(group) = canvas.nodes.get(group_index) else {
        return Vec::new();
    };
    if let Some(children) = &group.children {
        let index_of: HashMap<&str, usize> = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let mut indices: Vec<usize> = children
            .iter()
            .filter_map(|id| index_of.get(id.as_str()).copied())
            .filter(|&i| i != group_index)
            .collect();
        indices.sort_unstable();
        indices.dedup();
        return indices;
    }
    let (gx, gy) = (group.x, group.y);
    let (gx1, gy1) = (group.x + group.width, group.y + group.height);
    canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            if *index == group_index {
                return false;
            }
            let cx = node.x + node.width / 2.0;
            let cy = node.y + node.height / 2.0;
            cx >= gx && cx <= gx1 && cy >= gy && cy <= gy1
        })
        .map(|(index, _)| index)
        .collect()
}

// --- FR-012: явное членство групп (жест «втягивания») ---

/// Материализовать явный список детей группы из текущего membership
/// (легаси-группа становится группой с `children`). Повторный вызов — no-op.
pub fn group_materialize_children(canvas: &mut Canvas, group_index: usize) {
    let Some(group) = canvas.nodes.get(group_index) else {
        return;
    };
    if group.children.is_some() {
        return;
    }
    let ids: Vec<String> = group_children(canvas, group_index)
        .into_iter()
        .filter_map(|i| canvas.nodes.get(i).map(|n| n.id.clone()))
        .collect();
    if let Some(group) = canvas.nodes.get_mut(group_index) {
        group.children = Some(ids);
    }
}

/// Добавить ноды в группу по id (жест «втягивания», FR-012): список детей
/// материализуется (легаси — из геометрии) и расширяется новыми id.
/// Дубликаты и id самой группы игнорируются.
pub fn group_add_children(canvas: &mut Canvas, group_index: usize, node_ids: &[String]) {
    group_materialize_children(canvas, group_index);
    let Some(group) = canvas.nodes.get_mut(group_index) else {
        return;
    };
    let group_id = group.id.clone();
    let list = group.children.get_or_insert_with(Vec::new);
    for id in node_ids {
        if id == &group_id || list.contains(id) {
            continue;
        }
        list.push(id.clone());
    }
}

/// Убрать ноду из детей группы (жест «выноса», FR-012). У легаси-группы
/// список материализуется минус удаляемая нода. true — список изменился.
pub fn group_remove_child(canvas: &mut Canvas, group_index: usize, node_id: &str) -> bool {
    group_materialize_children(canvas, group_index);
    let Some(group) = canvas.nodes.get_mut(group_index) else {
        return false;
    };
    let Some(list) = group.children.as_mut() else {
        return false;
    };
    let before = list.len();
    list.retain(|id| id != node_id);
    list.len() != before
}

/// Расширить rect группы до bbox(дети) + `padding` по всем сторонам
/// (FR-012: авторасширение при вставке). false — детей нет / индекс невалиден.
pub fn group_expand_to_children(canvas: &mut Canvas, group_index: usize, padding: f32) -> bool {
    let children = group_children(canvas, group_index);
    if children.is_empty() {
        return false;
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for index in children {
        let Some(node) = canvas.nodes.get(index) else {
            continue;
        };
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(node.x + node.width);
        max_y = max_y.max(node.y + node.height);
    }
    if min_x > max_x {
        return false;
    }
    let Some(group) = canvas.nodes.get_mut(group_index) else {
        return false;
    };
    group.x = min_x - padding;
    group.y = min_y - padding;
    group.width = (max_x - min_x) + padding * 2.0;
    group.height = (max_y - min_y) + padding * 2.0;
    true
}

/// Все группы, содержащие ноду — явно (список `children`), геометрически
/// (легаси, центр внутри rect) или транзитивно через вложенные группы.
/// Роутинг связей использует список как исключение из препятствий: линк
/// к ноде внутри группы должен свободно проходить её границу (и границы
/// групп-предков), линк к группе в целом — исключён как конец связи.
/// Чистая функция; порядок — по возрастанию индексов.
pub fn enclosing_group_indices(canvas: &Canvas, node_index: usize) -> Vec<usize> {
    if canvas.nodes.get(node_index).is_none() {
        return Vec::new();
    }
    let mut result: Vec<usize> = Vec::new();
    // Обход вверх по вложенности: группа, найденная как содержащая текущую
    // ноду, сама может быть чьим-то ребёнком (вложенные группы)
    let mut frontier: Vec<usize> = vec![node_index];
    while let Some(current) = frontier.pop() {
        for (gi, group) in canvas.nodes.iter().enumerate() {
            if group.kind() != NodeKind::Group || result.contains(&gi) {
                continue;
            }
            if group_children(canvas, gi).contains(&current) {
                result.push(gi);
                frontier.push(gi);
            }
        }
    }
    result.sort_unstable();
    result
}

/// FR-012: план «мягкого раздвигания» — минимальные векторы выталкивания
/// для bbox'ов, пересекающихся с `rect` (по кратчайшей из четырёх осей
/// разрешения пересечения). Порядок входа сохранён (детерминизм); ноды без
/// пересечения в план не попадают. Чистая функция — тестируется без GPU.
pub fn plan_push_out(rect: [f32; 4], others: &[(usize, [f32; 4])]) -> Vec<(usize, [f32; 2])> {
    let (rx, ry, rw, rh) = (rect[0], rect[1], rect[2], rect[3]);
    others
        .iter()
        .filter_map(|&(index, bbox)| {
            let (bx, by, bw, bh) = (bbox[0], bbox[1], bbox[2], bbox[3]);
            // Пересечение (строгое) — иначе ноду не трогаем
            let overlap_w = (rx + rw).min(bx + bw) - rx.max(bx);
            let overlap_h = (ry + rh).min(by + bh) - ry.max(by);
            if overlap_w <= 0.0 || overlap_h <= 0.0 {
                return None;
            }
            // Минимальный выталкивающий вектор из четырёх осевых вариантов:
            // вправо (за правый край rect), влево, вниз, вверх
            let right = rx + rw - bx;
            let left = bx + bw - rx;
            let down = ry + rh - by;
            let up = by + bh - ry;
            let dx = if right <= left { right } else { -left };
            let dy = if down <= up { down } else { -up };
            let delta = if dx.abs() <= dy.abs() {
                [dx, 0.0]
            } else {
                [0.0, dy]
            };
            Some((index, delta))
        })
        .collect()
}

/// FR-011: индексы потомков ноды по исходящим рёбрам (поддерево mindmap,
/// корень не включается). Направление — от родителя к ребёнку
/// (`from_node → to_node`); циклы отсекаются (visited); порядок — BFS от
/// корня, соседи в порядке следования рёбер. Чистая функция.
pub fn subtree_ids(canvas: &Canvas, root: usize) -> Vec<usize> {
    if canvas.nodes.get(root).is_none() {
        return Vec::new();
    }
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut visited: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(root);
    while let Some(node) = queue.pop_front() {
        let Some(node_ref) = canvas.nodes.get(node) else {
            continue;
        };
        let node_id = node_ref.id.as_str();
        for edge in &canvas.edges {
            if edge.from_node != node_id {
                continue;
            }
            let Some(&child) = index_of.get(edge.to_node.as_str()) else {
                continue;
            };
            if child == root || visited.contains(&child) {
                continue;
            }
            visited.push(child);
            queue.push_back(child);
        }
    }
    visited.retain(|&i| i != root);
    visited
}

/// FR-011: родитель ноды в поддереве mindmap — from-нода ПЕРВОГО входящего
/// ребра (детерминизм: порядок рёбер модели). None — корень/нет входящих.
pub fn parent_index(canvas: &Canvas, node_index: usize) -> Option<usize> {
    let node = canvas.nodes.get(node_index)?;
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    canvas
        .edges
        .iter()
        .find(|edge| edge.to_node == node.id)
        .and_then(|edge| index_of.get(edge.from_node.as_str()).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_scene() -> Canvas {
        // Группа 0..400 × 0..300; дети: внутри, на границе (центр), снаружи,
        // вложенная группа. Индекс группы — 0.
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::group("g", 0.0, 0.0, 400.0, 300.0));
        canvas
            .nodes
            .push(Node::file("in", "C:/in.png", 100.0, 100.0, 50.0, 50.0));
        // Центр на правой границе (x = 400): центр 400..350 — включительно
        canvas
            .nodes
            .push(Node::file("edge", "C:/edge.png", 375.0, 100.0, 50.0, 50.0));
        canvas
            .nodes
            .push(Node::file("out", "C:/out.png", 500.0, 100.0, 50.0, 50.0));
        canvas
            .nodes
            .push(Node::group("nested", 50.0, 50.0, 100.0, 80.0));
        canvas
    }

    /// Дети группы: центр внутри rect (границы включительно), снаружи — нет,
    /// сама группа не считается своим ребёнком; вложенная группа — дитя.
    #[test]
    fn group_children_membership() {
        let canvas = group_scene();
        let children = group_children(&canvas, 0);
        assert_eq!(
            children,
            vec![1, 2, 4],
            "in + edge(центр на границе) + nested"
        );
        // Невалидный индекс — пусто, без паники
        assert!(group_children(&canvas, 99).is_empty());
    }

    /// Группы-предки ноды: геометрические (легаси) и через вложенность,
    /// транзитивно вверх. Нода вне групп — пусто; невалидный индекс — пусто.
    #[test]
    fn enclosing_groups_transitive() {
        let mut canvas = group_scene();
        // deep внутри nested, nested внутри g: предки deep — обе группы
        canvas
            .nodes
            .push(Node::file("deep", "C:/deep.png", 60.0, 60.0, 20.0, 20.0));
        assert_eq!(enclosing_group_indices(&canvas, 5), vec![0, 4]);
        // in — центр (125,125) внутри g И внутри nested [50..150 × 50..130]
        assert_eq!(enclosing_group_indices(&canvas, 1), vec![0, 4]);
        // edge — центр (400,125) только в g (nested правее/левее не достаёт)
        assert_eq!(enclosing_group_indices(&canvas, 2), vec![0]);
        // out снаружи — ни одной
        assert_eq!(enclosing_group_indices(&canvas, 3), Vec::<usize>::new());
        assert_eq!(enclosing_group_indices(&canvas, 99), Vec::<usize>::new());
        // Явное членство: g.children = [out] — out становится ребёнком g,
        // а геометрические дети g (in/edge/nested) теряют членство
        if let Some(group) = canvas.nodes.get_mut(0) {
            group.children = Some(vec!["out".to_owned()]);
        }
        assert_eq!(enclosing_group_indices(&canvas, 3), vec![0]);
        // in теряет членство в g (явный список), но nested (легаси-геометрия)
        // по-прежнему содержит его центр
        assert_eq!(enclosing_group_indices(&canvas, 1), vec![4]);
    }

    /// Вложенная группа сдвигается как обычная нода; её собственные дети
    /// при сдвиге НЕ перевычисляются рекурсивно — каждая нода сдвигается
    /// ровно один раз (двойной сдвиг был бы виден по величине дельты).
    #[test]
    fn translate_group_moves_children_once() {
        let mut canvas = group_scene();
        // deep лежит внутри nested, а nested внутри g — deep дитя обеих,
        // но translate_group(g) обязан сдвинуть его ровно один раз
        canvas
            .nodes
            .push(Node::file("deep", "C:/deep.png", 60.0, 60.0, 20.0, 20.0));

        let moved = canvas.translate_group(0, 10.0, -5.0);
        // Группа + in + edge + nested + deep (deep — тоже дитя g: центр внутри)
        assert_eq!(moved, vec![0, 1, 2, 4, 5]);
        assert_eq!((canvas.nodes[0].x, canvas.nodes[0].y), (10.0, -5.0));
        assert_eq!((canvas.nodes[1].x, canvas.nodes[1].y), (110.0, 95.0));
        assert_eq!((canvas.nodes[2].x, canvas.nodes[2].y), (385.0, 95.0));
        // Снаружи — на месте
        assert_eq!((canvas.nodes[3].x, canvas.nodes[3].y), (500.0, 100.0));
        // Вложенная группа сдвинулась как нода
        assert_eq!((canvas.nodes[4].x, canvas.nodes[4].y), (60.0, 45.0));
        // deep сдвинут РОВНО ОДИН раз: (60+10, 60-5), а не дважды
        assert_eq!((canvas.nodes[5].x, canvas.nodes[5].y), (70.0, 55.0));

        // Невалидный индекс — ничего не сдвигается
        assert!(canvas.translate_group(99, 1.0, 1.0).is_empty());
        assert_eq!((canvas.nodes[0].x, canvas.nodes[0].y), (10.0, -5.0));
    }

    /// Удаление группы детей не удаляет (как Obsidian): remove_node каскадит
    /// только связи удаляемой ноды.
    #[test]
    fn removing_group_keeps_children() {
        let mut canvas = group_scene();
        let removed = canvas.remove_node(0).expect("группа удалена");
        assert_eq!(removed.id, "g");
        assert_eq!(canvas.nodes.len(), 4);
        assert!(canvas.nodes.iter().all(|node| node.id != "g"));
    }

    /// CR-001: remove_nodes — порядок удаления от больших индексов к
    /// меньшим (сдвиги не ломают адресацию), каскад связей по всем
    /// удалённым, дубликаты и невалидные индексы игнорируются.
    #[test]
    fn remove_nodes_bulk_cascades_and_orders() {
        let mut canvas = group_scene();
        // in(1) → out(3), edge(2) → out(3): связи затрагивают удаляемые
        canvas.add_edge(Edge::new("edge-1", "in", None, "out", None));
        canvas.add_edge(Edge::new("edge-2", "edge", None, "out", None));
        // Удаляем in(1), edge(2) — c дубликатом; out(3) остаётся
        let removed = canvas.remove_nodes(&[1, 2, 1]);
        assert_eq!(
            removed
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["in", "edge"],
            "удалены по возрастанию индексов, дубликат один раз"
        );
        assert_eq!(canvas.nodes.len(), 3, "остались g, out, nested");
        // Связи удалённых нод ушли каскадно, инцидентные out-уцелевшие — нет:
        // edge-1 (in→out) и edge-2 (edge→out) оба касаются удалённых — их нет
        assert!(canvas.edges.is_empty(), "обе связи инцидентны удалённым");
        // Уцелевшая нода доступна по id
        assert!(canvas.node("out").is_some());
        // Невалидные индексы — пусто, без паники
        assert!(canvas.remove_nodes(&[99, 100]).is_empty());
        // Пустой набор — пусто
        assert!(canvas.remove_nodes(&[]).is_empty());
    }

    /// Node::group: тип, отсутствие file/text, координаты как заданы.
    #[test]
    fn group_constructor_fields() {
        let group = Node::group("g1", 10.0, 20.0, 400.0, 300.0);
        assert_eq!(group.kind(), NodeKind::Group);
        assert_eq!(group.node_type, "group");
        assert_eq!(group.file, None);
        assert_eq!(group.text, None);
        assert_eq!(group.label, None);
        assert_eq!(
            (group.x, group.y, group.width, group.height),
            (10.0, 20.0, 400.0, 300.0)
        );
    }

    /// Node::widget (M5): kind/Widget, canvasdesk-расширение с props, label —
    /// имя пакета; прочие поля пусты.
    #[test]
    fn widget_constructor_fields() {
        let mut props = Map::new();
        props.insert("city".to_owned(), Value::String("Moscow".to_owned()));
        let ext = CanvasdeskExt {
            widget_id: Some("com.canvasdesk.clock".to_owned()),
            props,
            expr: None,
        };
        let widget = Node::widget("w1", ext, "Clock", 5.0, 6.0, 320.0, 200.0);
        assert_eq!(widget.kind(), NodeKind::Widget);
        assert_eq!(widget.node_type, "widget");
        let ext = widget.canvasdesk.as_ref().expect("canvasdesk задан");
        assert_eq!(ext.widget_id.as_deref(), Some("com.canvasdesk.clock"));
        assert_eq!(
            ext.props.get("city"),
            Some(&Value::String("Moscow".to_owned()))
        );
        assert_eq!(widget.label.as_deref(), Some("Clock"));
        assert_eq!(widget.file, None);
        assert_eq!(widget.text, None);
        assert_eq!(
            (widget.x, widget.y, widget.width, widget.height),
            (5.0, 6.0, 320.0, 200.0)
        );
    }

    /// Round-trip виджет-ноды (M5): тип, canvasdesk-расширение и соседние
    /// unknown-поля переживают сериализацию без потерь (SPEC §5.1 — файл
    /// остаётся валидным для сторонних редакторов).
    #[test]
    fn widget_node_round_trip() {
        let mut props = Map::new();
        props.insert("text".to_owned(), Value::String("покупки".to_owned()));
        let ext = CanvasdeskExt {
            widget_id: Some("com.example.clock".to_owned()),
            props,
            expr: None,
        };
        let mut widget = Node::widget("w9", ext, "Clock", 0.0, 0.0, 320.0, 200.0);
        // Стороннее поле уровня ноды — сохраняется как unknown
        widget
            .extra
            .insert("futureField".to_owned(), Value::Bool(true));

        let json = serde_json::to_string(&widget).expect("сериализация");
        let back: Node = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(back, widget, "round-trip без потерь");
        assert_eq!(back.kind(), NodeKind::Widget);
        assert_eq!(
            back.extra.get("futureField"),
            Some(&Value::Bool(true)),
            "unknown-поле рядом с canvasdesk не потеряно"
        );
    }

    /// Виджет-строка из JSON-сырца: парсинг поля `canvasdesk` и строки
    /// `type: "widget"` (формат SPEC §5.1).
    #[test]
    fn widget_node_parsed_from_spec_json() {
        let raw = r#"{
            "id": "n1", "type": "widget", "x": 10, "y": 20, "width": 300, "height": 180,
            "canvasdesk": { "widgetId": "com.canvasdesk.sticker", "props": { "text": "привет" } }
        }"#;
        let node: Node = serde_json::from_str(raw).expect("парсинг SPEC-примера");
        assert_eq!(node.kind(), NodeKind::Widget);
        let ext = node.canvasdesk.as_ref().expect("canvasdesk");
        assert_eq!(ext.widget_id.as_deref(), Some("com.canvasdesk.sticker"));
        assert_eq!(
            ext.props.get("text").and_then(Value::as_str),
            Some("привет")
        );
    }

    // --- FR-011: mindmap (subtree_ids / parent_index / collapsed) ---

    fn mindmap_scene() -> Canvas {
        // root → child1, root → child2; child1 → grand; side → root (входящее
        // ребро из внешней ноды — не часть поддерева root)
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("root", "root", 0.0, 0.0));
        canvas.nodes.push(Node::text("child1", "c1", 300.0, 0.0));
        canvas.nodes.push(Node::text("child2", "c2", 300.0, 150.0));
        canvas.nodes.push(Node::text("grand", "g", 600.0, 0.0));
        canvas.nodes.push(Node::text("side", "s", -300.0, 0.0));
        canvas.add_edge(Edge::new("e1", "root", None, "child1", None));
        canvas.add_edge(Edge::new("e2", "root", None, "child2", None));
        canvas.add_edge(Edge::new("e3", "child1", None, "grand", None));
        canvas.add_edge(Edge::new("e4", "side", None, "root", None));
        canvas
    }

    #[test]
    fn subtree_bfs_directional() {
        let canvas = mindmap_scene();
        // Поддерево root: child1, child2, grand; side НЕ входит (ребро
        // направлено В root, а поддерево идёт ТОЛЬКО по исходящим)
        assert_eq!(subtree_ids(&canvas, 0), vec![1, 2, 3]);
        // Поддерево child1 — только grand
        assert_eq!(subtree_ids(&canvas, 1), vec![3]);
        // Лист — пусто; невалидный индекс — пусто
        assert!(subtree_ids(&canvas, 3).is_empty());
        assert!(subtree_ids(&canvas, 99).is_empty());
    }

    #[test]
    fn subtree_cycle_terminates() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 100.0, 0.0));
        canvas.add_edge(Edge::new("e1", "a", None, "b", None));
        canvas.add_edge(Edge::new("e2", "b", None, "a", None));
        // Цикл a↔b: обход завершается, b — единственный потомок a
        assert_eq!(subtree_ids(&canvas, 0), vec![1]);
    }

    #[test]
    fn parent_first_incoming_edge() {
        let canvas = mindmap_scene();
        // child1: единственное входящее — root (индекс 0)
        assert_eq!(parent_index(&canvas, 1), Some(0));
        // root: первое входящее ребро e4 от side (индекс 4)
        assert_eq!(parent_index(&canvas, 0), Some(4));
        // side: входящих нет — корень канваса
        assert_eq!(parent_index(&canvas, 4), None);
        assert_eq!(parent_index(&canvas, 99), None);
    }

    #[test]
    fn collapsed_round_trip() {
        let mut node = Node::text("n", "ветка", 0.0, 0.0);
        // None — поле не попадает в JSON (чистый файл для чужих редакторов)
        let json = serde_json::to_string(&node).expect("сериализация");
        assert!(!json.contains("collapsed"), "None не сериализуется");
        // Some(true) — сериализуется и восстанавливается
        node.collapsed = Some(true);
        let json = serde_json::to_string(&node).expect("сериализация");
        assert!(json.contains("collapsed"), "Some(true) сериализуется");
        let back: Node = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(back.collapsed, Some(true));
        // Чужой файл с "collapsed": true парсится (расширение SPEC §5.1)
        let raw = r#"{ "id": "x", "type": "text", "x": 0, "y": 0,
            "width": 10, "height": 10, "collapsed": true }"#;
        let parsed: Node = serde_json::from_str(raw).expect("парсинг расширения");
        assert_eq!(parsed.collapsed, Some(true));
    }

    // --- FR-012: явное членство групп ---

    /// Явный список `children` замещает геометрию: нода, случайно лежащая
    /// поверх группы, ребёнком НЕ становится (главный регресс FR-012).
    #[test]
    fn explicit_children_ignore_random_overlap() {
        let mut canvas = group_scene();
        // Легаси-группа: геометрический фолбэк
        assert_eq!(group_children(&canvas, 0), vec![1, 2, 4]);
        // Материализуем явный список: те же дети
        group_materialize_children(&mut canvas, 0);
        assert_eq!(
            canvas.nodes[0].children.as_deref(),
            Some(&["in".to_owned(), "edge".to_owned(), "nested".to_owned()][..])
        );
        // Случайно занесённая поверх группы нода ребёнком НЕ становится
        canvas
            .nodes
            .push(Node::file("random", "C:/r.png", 100.0, 100.0, 50.0, 50.0));
        assert_eq!(
            group_children(&canvas, 0),
            vec![1, 2, 4],
            "random не подвязан"
        );
        // Явная вставка жестом — теперь ребёнок
        group_add_children(&mut canvas, 0, &["random".to_owned()]);
        assert_eq!(group_children(&canvas, 0), vec![1, 2, 4, 5]);
        // Повторная вставка — дубликат игнорируется
        group_add_children(&mut canvas, 0, &["random".to_owned()]);
        assert_eq!(group_children(&canvas, 0), vec![1, 2, 4, 5]);
    }

    /// Вынос ребёнка: список материализуется минус нода; translate_group
    /// с явным списком двигает детей ровно один раз.
    #[test]
    fn remove_child_and_translate_explicit() {
        let mut canvas = group_scene();
        group_materialize_children(&mut canvas, 0);
        // Вложенная группа nested вышла из состава
        assert!(group_remove_child(&mut canvas, 0, "nested"));
        assert!(
            !group_remove_child(&mut canvas, 0, "nested"),
            "повтор — no-op"
        );
        assert_eq!(group_children(&canvas, 0), vec![1, 2]);
        // Вложенная группа ПОСЛЕ выноса лежит поверх g, но не ребёнок
        // (геометрия больше не решает) — и nested своих детей не теряет
        // translate двигает только in/edge + саму группу
        let moved = canvas.translate_group(0, 10.0, 10.0);
        assert_eq!(moved, vec![0, 1, 2], "nested и его дети не тронуты");
    }

    /// Авторасширение: rect группы = bbox(дети) + padding по всем сторонам.
    #[test]
    fn expand_to_children_bbox() {
        let mut canvas = group_scene();
        group_materialize_children(&mut canvas, 0);
        // Дети: in (50..150 × 50..130), edge (375..425 × 100..150)
        assert!(group_expand_to_children(&mut canvas, 0, 40.0));
        let g = &canvas.nodes[0];
        assert_eq!((g.x, g.y, g.width, g.height), (10.0, 10.0, 455.0, 180.0));
        // Без детей — false, rect не меняется
        let mut empty = Canvas::default();
        empty.nodes.push(Node::group("g2", 0.0, 0.0, 100.0, 100.0));
        assert!(!group_expand_to_children(&mut empty, 0, 40.0));
    }

    /// Мягкое раздвигание: минимальный осевой вектор, отсутствие
    /// пересечения — нода не в плане, детерминизм.
    #[test]
    fn push_out_minimal_axis() {
        // Нода частично заезжает справа-снизу — вытолкнута по кратчайшей оси
        let rect = [0.0, 0.0, 400.0, 300.0];
        let others = vec![
            (1, [380.0, 100.0, 100.0, 80.0]), // пересекается справа (20 px)
            (2, [100.0, 280.0, 100.0, 80.0]), // пересекается снизу (20 px)
            (3, [500.0, 500.0, 100.0, 80.0]), // без пересечения
        ];
        let plan = plan_push_out(rect, &others);
        assert_eq!(plan.len(), 2, "без пересечения — нет в плане");
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        // (1): вправо 20 против вверх 180 → dx = +20
        assert_eq!(by_id[&1], [20.0, 0.0]);
        // (2): целиком внутри по x (влево/вправо 200/300 px), вниз — 20 px
        assert_eq!(by_id[&2], [0.0, 20.0]);
        // Повтор — детерминизм
        assert_eq!(plan_push_out(rect, &others), plan);
    }
}
