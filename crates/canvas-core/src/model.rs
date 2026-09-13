//! Модель данных `.canvas` (JSON Canvas 1.0, SPEC §5.1).
//!
//! Совместимость с jsoncanvas.org: неизвестные поля нод, связей и корня
//! сохраняются в `extra` (serde flatten) и не теряются при round-trip;
//! неизвестные типы нод не ломают парсинг (`node_type` — строка).

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

/// Расширение `canvasdesk` ноды-виджета (M5, SPEC §5.1/§7.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasdeskExt {
    #[serde(rename = "widgetId")]
    pub widget_id: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
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
            extra: Map::new(),
        }
    }

    /// Нода-группа: рамка с подписью (`label`). Дети определяются геометрией
    /// (центр ноды внутри rect группы, см. `group_children`) — формат
    /// `.canvas` родительские связи не хранит.
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

/// Индексы детей группы: ноды (кроме самой группы), чей центр лежит внутри
/// rect группы (границы включительно). Вложенные группы считаются обычными
/// нодами — рекурсии нет (v1 групп). Чистая функция — тестируется без GPU.
pub fn group_children(canvas: &Canvas, group_index: usize) -> Vec<usize> {
    let Some(group) = canvas.nodes.get(group_index) else {
        return Vec::new();
    };
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
            widget_id: "com.canvasdesk.clock".to_owned(),
            props,
        };
        let widget = Node::widget("w1", ext, "Clock", 5.0, 6.0, 320.0, 200.0);
        assert_eq!(widget.kind(), NodeKind::Widget);
        assert_eq!(widget.node_type, "widget");
        let ext = widget.canvasdesk.as_ref().expect("canvasdesk задан");
        assert_eq!(ext.widget_id, "com.canvasdesk.clock");
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
            widget_id: "com.example.clock".to_owned(),
            props,
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
        assert_eq!(ext.widget_id, "com.canvasdesk.sticker");
        assert_eq!(
            ext.props.get("text").and_then(Value::as_str),
            Some("привет")
        );
    }
}
