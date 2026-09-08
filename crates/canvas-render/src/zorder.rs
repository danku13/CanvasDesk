//! Z-порядок кадра: сегментация видимых нод для чередования отрисовки (фикс
//! наложения текста и тамбнейлов поверх карточек переднего плана).
//!
//! Проблема: карточки, тамбнейлы и текст рисовались тремя сплошными проходами
//! — все карточки, потом все тамбнейлы, потом весь текст. Тамбнейл фоновой
//! ноды (иконка Word на скриншоте) оказывался поверх карточек переднего
//! плана, а текст фоновой заметки — поверх чужих карточек и текста.
//!
//! Решение: z-порядок = порядок нод в `Canvas.nodes` (выдача spatial index
//! отсортирована по возрастанию). Кадр бьётся на сегменты так, чтобы текст и
//! тамбнейл ноды рисовались после её карточки, но ДО карточек, перекрывающих
//! её. Сегмент кадра отрисовывается как: карточки (диапазон инстансов) →
//! тамбнейлы (диапазон) → тексты (текст-группа = один TextRenderer из пула
//! `TextSystem`).
//!
//! Модуль — чистая логика без GPU: тестируется юнит-тестами.

/// Прямоугольники пересекаются (строгое перекрытие площадью > 0;
/// касание краями перекрытием не считается).
fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// Сегмент отрисовки: диапазон видимых нод (z-порядок) и текст-группа.
///
/// Рендерер рисует сегмент как: инстансы карточек диапазона нод (карточки +
/// квады подсветки/каретки на z-позициях нод) → инстансы тамбнейлов →
/// текст-группу `group` (если есть).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZSegment {
    /// Диапазон позиций в выдаче видимых нод (не индексы `Canvas.nodes`).
    pub nodes: std::ops::Range<usize>,
    /// Текст-группа сегмента: тексты нод, рисуемые после карточек сегмента.
    /// None — текстов в сегменте нет (сегмент разорван ради тамбнейла или
    /// потолка групп; тексты таких сегментов попадают в финальную группу).
    pub group: Option<usize>,
}

/// План z-порядка кадра: сегменты + текст-группы.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ZPlan {
    pub segments: Vec<ZSegment>,
    /// Текст-группы: `text_groups[g]` — позиции видимых нод, чьи тексты
    /// рисуются проходом `g`. Последняя группа — финальная: в неё попадают
    /// тексты нод финального сегмента, «перелив» из сегментов сверх потолка
    /// `max_text_groups`, а также оверлеи приложения и HUD (добавляет
    /// рендерер).
    pub text_groups: Vec<Vec<usize>>,
}

impl ZPlan {
    /// Число текст-групп (включая финальную) — размер пула TextRenderer.
    pub fn group_count(&self) -> usize {
        self.text_groups.len()
    }

    /// Идентификатор финальной текст-группы (оверлеи + HUD).
    pub fn final_group(&self) -> usize {
        self.text_groups.len().saturating_sub(1)
    }
}

/// Собрать план z-порядка по видимым нодам (в z-порядке).
///
/// * `rects` — world-прямоугольники нод `[x0, y0, x1, y1]`;
/// * `has_text` — у ноды есть текстовые области (заголовок/тело/буфер
///   редактора) — их надо рисовать до перекрывающих карточек;
/// * `has_thumb` — у ноды есть тамбнейл в атласе (рисуется в её сегменте);
/// * `max_text_groups` — потолок текст-групп (включая финальную); сегменты
///   сверх потолка не получают своей группы — их тексты попадают в финальную
///   (патологически глубокие каскады перекрытий, артефакт только там).
///
/// Разрыв сегмента происходит перед карточкой, перекрывающей любую ноду
/// текущего сегмента, у которой есть текст или тамбнейл: содержимое такой
/// ноды обязано оказаться под перекрывающей карточкой.
pub fn plan_z_order(
    rects: &[[f32; 4]],
    has_text: &[bool],
    has_thumb: &[bool],
    max_text_groups: usize,
) -> ZPlan {
    let count = rects.len().min(has_text.len()).min(has_thumb.len());
    // Ноды текущего сегмента с контентом (текст/тамбнейл) — кандидаты на
    // «заслонение» следующими карточками.
    let mut content_nodes: Vec<usize> = Vec::new();
    // Текстовые ноды текущего сегмента (для его группы).
    let mut seg_texts: Vec<usize> = Vec::new();
    // Тексты сегментов, не получивших группу (потолок) — в финальную группу.
    let mut overflow: Vec<usize> = Vec::new();
    let mut segments: Vec<ZSegment> = Vec::new();
    // Промежуточные группы (финальная не входит в сравнение с потолком).
    let mut text_groups: Vec<Vec<usize>> = Vec::new();
    let mut seg_start = 0usize;
    for i in 0..count {
        let covered = content_nodes
            .iter()
            .any(|&p| rects_intersect(rects[p], rects[i]));
        if covered && i > seg_start {
            // Закрыть сегмент [seg_start..i): тексты его нод рисуются
            // до карточки i. Группа — если тексты есть и потолок не исчерпан.
            let group = if !seg_texts.is_empty() && text_groups.len() + 1 < max_text_groups {
                text_groups.push(seg_texts.clone());
                Some(text_groups.len() - 1)
            } else {
                overflow.extend(seg_texts.iter().copied());
                None
            };
            segments.push(ZSegment {
                nodes: seg_start..i,
                group,
            });
            seg_start = i;
            content_nodes.clear();
            seg_texts.clear();
        }
        if has_text[i] || has_thumb[i] {
            content_nodes.push(i);
        }
        if has_text[i] {
            seg_texts.push(i);
        }
    }
    // Финальный сегмент — всегда с группой: тексты его нод + перелив,
    // поверх всего кадра рисуются ещё оверлеи приложения и HUD.
    let mut final_group = overflow;
    final_group.extend(seg_texts.iter().copied());
    let final_id = text_groups.len();
    text_groups.push(final_group);
    segments.push(ZSegment {
        nodes: seg_start..count,
        group: Some(final_id),
    });

    ZPlan {
        segments,
        text_groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Прямоугольник [x, y, w, h] → [x0, y0, x1, y1].
    fn rect(x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [x, y, x + w, y + h]
    }

    fn seg(nodes: std::ops::Range<usize>, group: Option<usize>) -> ZSegment {
        ZSegment { nodes, group }
    }

    fn plan(rects: &[[f32; 4]], texts: &[bool]) -> ZPlan {
        let thumbs = vec![false; rects.len()];
        plan_z_order(rects, texts, &thumbs, 16)
    }

    /// Без перекрытий — один финальный сегмент, одна группа со всеми текстами.
    #[test]
    fn disjoint_nodes_single_segment() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(200.0, 0.0, 100.0, 50.0)];
        let zplan = plan(&rects, &[true, true]);
        assert_eq!(zplan.segments, vec![seg(0..2, Some(0))]);
        assert_eq!(zplan.text_groups, vec![vec![0, 1]]);
        assert_eq!(zplan.final_group(), 0);
    }

    /// B перекрывает A, обе с текстом: текст A — в группе ДО карточки B
    /// (сегмент [A) с группой {A}; финальный [B..] с группой {B}).
    #[test]
    fn overlap_splits_text_before_covering_card() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 10.0, 100.0, 50.0)];
        let zplan = plan(&rects, &[true, true]);
        assert_eq!(zplan.segments, vec![seg(0..1, Some(0)), seg(1..2, Some(1))]);
        assert_eq!(zplan.text_groups, vec![vec![0], vec![1]]);
    }

    /// Нижняя нода без текста: перекрытие не рвёт сегмент (рисовать между
    /// ними нечего — только карточки в порядке инстансов).
    #[test]
    fn overlap_without_text_keeps_single_segment() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 10.0, 100.0, 50.0)];
        let zplan = plan(&rects, &[false, true]);
        assert_eq!(zplan.segments, vec![seg(0..2, Some(0))]);
        assert_eq!(zplan.text_groups, vec![vec![1]]);
    }

    /// Верхняя карточка без контента всё равно рвёт сегмент: текст нижней
    /// ноды обязан уйти под перекрывающую карточку.
    #[test]
    fn covering_card_without_content_splits() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 10.0, 100.0, 50.0)];
        let zplan = plan(&rects, &[true, false]);
        assert_eq!(zplan.segments, vec![seg(0..1, Some(0)), seg(1..2, Some(1))]);
        assert_eq!(zplan.text_groups, vec![vec![0], Vec::<usize>::new()]);
    }

    /// Тамбнейл без текста тоже требует разрыва: иконка фоновой карточки
    /// не должна рисоваться поверх перекрывающей карточки (кейс скриншота).
    #[test]
    fn thumbnail_only_node_splits_on_cover() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 10.0, 100.0, 50.0)];
        let thumbs = [true, false];
        let zplan = plan_z_order(&rects, &[false, false], &thumbs, 16);
        // сегмент [0..1) разорван ради тамбнейла, но текстовой группы не имеет;
        // тексты — только финальная (пустая: текстов нет вовсе)
        assert_eq!(zplan.segments, vec![seg(0..1, None), seg(1..2, Some(0))]);
        assert_eq!(zplan.text_groups, vec![Vec::<usize>::new()]);
    }

    /// Каскад A < B < C (каждая перекрывает предыдущую): три сегмента,
    /// тексты в порядке z — A, B, C.
    #[test]
    fn cascade_three_levels() {
        let rects = [
            rect(0.0, 0.0, 100.0, 50.0),
            rect(50.0, 10.0, 100.0, 50.0),
            rect(75.0, 20.0, 100.0, 50.0),
        ];
        let zplan = plan(&rects, &[true, true, true]);
        assert_eq!(
            zplan.segments,
            vec![seg(0..1, Some(0)), seg(1..2, Some(1)), seg(2..3, Some(2))]
        );
        assert_eq!(zplan.text_groups, vec![vec![0], vec![1], vec![2]]);
    }

    /// Касание краями — не перекрытие: один сегмент.
    #[test]
    fn touching_edges_do_not_split() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(100.0, 50.0, 100.0, 50.0)];
        let zplan = plan(&rects, &[true, true]);
        assert_eq!(zplan.segments.len(), 1);
    }

    /// Потолок групп: сегменты сверх лимита теряют свою группу, их тексты
    /// сливаются в финальную (перелив).
    #[test]
    fn group_cap_overflows_to_final() {
        let rects = [
            rect(0.0, 0.0, 100.0, 50.0),
            rect(50.0, 0.0, 100.0, 50.0),
            rect(100.0, 0.0, 100.0, 50.0),
            rect(150.0, 0.0, 100.0, 50.0),
        ];
        // max_text_groups = 2: одна промежуточная + финальная
        let thumbs = vec![false; 4];
        let zplan = plan_z_order(&rects, &[true; 4], &thumbs, 2);
        // сегменты: [0..1) с группой, [1..2) и [2..3) без (потолок), [3..4) финал
        assert_eq!(
            zplan.segments,
            vec![
                seg(0..1, Some(0)),
                seg(1..2, None),
                seg(2..3, None),
                seg(3..4, Some(1)),
            ]
        );
        // группы: {0}, финальная {1, 2, 3} (перелив + тексты финального сегмента)
        assert_eq!(zplan.text_groups, vec![vec![0], vec![1, 2, 3]]);
    }

    /// Пустая сцена — один пустой финальный сегмент и пустая группа.
    #[test]
    fn empty_scene() {
        let zplan = plan(&[], &[]);
        assert_eq!(zplan.segments, vec![seg(0..0, Some(0))]);
        assert_eq!(zplan.text_groups, vec![Vec::<usize>::new()]);
    }

    /// Нода без текста в финальном сегменте не попадает в группу.
    #[test]
    fn final_group_only_text_nodes() {
        let rects = [rect(0.0, 0.0, 100.0, 50.0), rect(200.0, 0.0, 10.0, 10.0)];
        let zplan = plan(&rects, &[true, false]);
        assert_eq!(zplan.text_groups, vec![vec![0]]);
    }

    /// Промежуточный сегмент без текста (разрыв ради тамбнейла) не создаёт
    /// пустую группу — группа только у финального.
    #[test]
    fn segment_without_text_has_no_group() {
        // A с тамбнейлом, B с текстом перекрывает A, C с текстом не перекрывает
        let rects = [
            rect(0.0, 0.0, 100.0, 50.0),
            rect(50.0, 10.0, 100.0, 50.0),
            rect(300.0, 0.0, 100.0, 50.0),
        ];
        let thumbs = [true, false, false];
        let zplan = plan_z_order(&rects, &[false, true, true], &thumbs, 16);
        assert_eq!(zplan.segments, vec![seg(0..1, None), seg(1..3, Some(0))]);
        assert_eq!(zplan.text_groups, vec![vec![1, 2]]);
    }
}
