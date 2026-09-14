//! LOD-планировщик виджетов (SPEC §7.6/§6.2, план M5 §4.5): live/snapshot/
//! placeholder по zoom+видимости+перекрытию, лимит live-инстансов с LRU,
//! гистерезис порога, периодический refresh снапшотов.
//!
//! Чистая функция `plan_frame` без Win32/WebView2 — тестируется на Linux;
//! host (Windows) применяет решения: SetWindowPos/show/suspend/destroy.

use std::collections::HashMap;

/// Вход в live: zoom ≥ 0.27 (гистерезис ±0.02 от порога 0.25, SPEC §7.6).
pub const LIVE_ENTER_ZOOM: f32 = 0.27;
/// Выход из live: zoom < 0.23 (выход ниже порога входа — защита от дребезга).
pub const LIVE_EXIT_ZOOM: f32 = 0.23;
/// Лимит одновременно live (живых, несусpended) инстансов — конфиг (SPEC: 6).
pub const LIVE_LIMIT: usize = 6;
/// Пул suspended-инстансов за пределами live-лимита (снапшоты + refresh).
pub const SNAPSHOT_POOL: usize = 4;
/// Всего инстансов: live + suspended-пул; свыше — destroy (память WebView2).
pub const MAX_INSTANCES: usize = LIVE_LIMIT + SNAPSHOT_POOL;
/// Период refresh снапшота видимых suspended-виджетов (SPEC: раз в 5 с).
pub const SNAPSHOT_REFRESH_SECS: u64 = 5;
/// Refresh нужен только для видимых при zoom ≥ 0.25 (ниже — мелко на экране).
pub const SNAPSHOT_MIN_ZOOM: f32 = 0.25;

/// Входные данные по одной виджет-ноде на кадр (заполняет менеджер).
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetLodInput {
    pub node_id: String,
    /// Нода попадает в viewport (culling).
    pub visible: bool,
    /// Перекрыта оверлеем/рамкой выделения/протягиванием ребра (airspace П7).
    pub overlaid: bool,
    /// Пакет установлен и манифест валиден.
    pub package_ok: bool,
    /// WebView2-рантайм доступен.
    pub runtime_ok: bool,
    /// Контроллер для ноды существует (live или suspended).
    pub has_instance: bool,
    /// Нода сейчас в live (для гистерезиса и final_capture).
    pub was_live: bool,
    /// Зум камеры (одинаков для всех, но поле — для тестируемости).
    pub zoom: f32,
    /// LRU-тик последнего «смотрения» (видимость/взаимодействие) — приоритет.
    pub last_seen: u64,
}

/// Целевое состояние виджета на кадр.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Live-инстанс: HWND видим, синхронизируется с камерой.
    Live,
    /// Инстанс есть, но скрыт и приостановлен (TrySuspend); рисуем снапшот.
    Suspended,
    /// Инстанса нет и не нужен (далёкий зум/невидимость): рисуем снапшот
    /// (если есть) либо заглушку.
    NoInstance,
    /// Инстанс есть, но за пределами пула — уничтожить (LRU-вытеснение).
    Destroy,
    /// Пакет бит/рантайм недоступен: серая заглушка (brokenLink-стиль).
    Placeholder,
    /// Пакет бит/рантайм недоступен, инстанс существовал — уничтожить.
    PlaceholderDestroy,
}

/// Решение по виджету на кадр.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDecision {
    pub node_id: String,
    pub target: Target,
    /// Уход из live: финальный CapturePreview ДО скрытия (SPEC §7.6).
    pub final_capture: bool,
    /// Пора освежить снапшот suspended-виджета (resume→capture→suspend).
    pub refresh_snapshot: bool,
}

/// Режим для рендера: что рисуем в области контента.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Live: квад не рисуем — HWND над канвасом.
    Live,
    /// Snapshot/заглушка: квад в области контента.
    Snapshot,
    /// Битый пакет: серая заглушка.
    Broken,
}

impl WidgetDecision {
    /// Режим рендера (renderer не знает про host — только это).
    pub fn render_mode(&self) -> RenderMode {
        match self.target {
            Target::Live => RenderMode::Live,
            Target::Suspended | Target::NoInstance | Target::Destroy => RenderMode::Snapshot,
            Target::Placeholder | Target::PlaceholderDestroy => RenderMode::Broken,
        }
    }
}

/// Планирование кадра (план M5 §4.5, алгоритм):
/// 1. Битые (пакет/рантайм) → Placeholder(±Destroy).
/// 2. Кандидаты в live: видимы, не перекрыты, zoom за гистерезисным порогом
///    (был live — выход 0.23, не был — вход 0.27); топ-LIVE_LIMIT по LRU.
/// 3. Держатели инстансов: кандидаты по last_seen (включая невидимых —
///    suspended для снапшотов), топ-MAX_INSTANCES; вылетевшие → Destroy.
/// 4. refresh: suspended + видим + zoom ≥ 0.25 + прошло 5 с с last_capture.
pub fn plan_frame(
    inputs: &[WidgetLodInput],
    tick: u64,
    last_capture: &HashMap<String, u64>,
) -> Vec<WidgetDecision> {
    // Индексы входов, отсортированные по LRU-приоритету (свежие раньше;
    // стабильная сортировка сохраняет порядок нод при равенстве).
    let mut by_lru: Vec<usize> = (0..inputs.len()).collect();
    by_lru.sort_by(|&a, &b| inputs[b].last_seen.cmp(&inputs[a].last_seen));

    // Держатели инстансов: рабочие (пакет+рантайм) виджеты по LRU.
    let mut instance_holders: Vec<usize> = Vec::new();
    for &i in &by_lru {
        if inputs[i].package_ok && inputs[i].runtime_ok {
            instance_holders.push(i);
            if instance_holders.len() == MAX_INSTANCES {
                break;
            }
        }
    }

    // Live: из держателей — подходящие (видимость/перекрытие/зум), топ-лимит.
    let mut live_set: Vec<usize> = Vec::new();
    for &i in &instance_holders {
        let w = &inputs[i];
        if live_eligible(w) {
            live_set.push(i);
            if live_set.len() == LIVE_LIMIT {
                break;
            }
        }
    }

    inputs
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let node_id = w.node_id.clone();
            if !w.package_ok || !w.runtime_ok {
                let target = if w.has_instance {
                    Target::PlaceholderDestroy
                } else {
                    Target::Placeholder
                };
                return WidgetDecision {
                    node_id,
                    target,
                    final_capture: w.has_instance && w.was_live,
                    refresh_snapshot: false,
                };
            }

            let holder = instance_holders.contains(&i);
            let is_live = live_set.contains(&i);

            if is_live {
                return WidgetDecision {
                    node_id,
                    target: Target::Live,
                    final_capture: false,
                    refresh_snapshot: false,
                };
            }

            // Не live: держим инстанс suspended либо уничтожаем
            let target = if holder {
                Target::Suspended
            } else {
                Target::Destroy
            };

            // Уход из live → финальный снимок до скрытия
            let final_capture = w.was_live && !is_live;

            // refresh: suspended, видим, не перекрыт, зум достаточен, протух.
            // CR-005: виджет без ЕДИНОГО снапшота (ключа нет в last_capture)
            // освежается немедленно — раньше он ждал SNAPSHOT_REFRESH_SECS
            // от старта приложения и всё это время показывал пустую карточку
            // вместо контента (часы — «--:--:--» или пустота).
            let never_captured = !last_capture.contains_key(&w.node_id);
            let last = last_capture.get(&w.node_id).copied().unwrap_or(0);
            let refresh_snapshot = target == Target::Suspended
                && w.visible
                && !w.overlaid
                && w.zoom >= SNAPSHOT_MIN_ZOOM
                && (never_captured || tick.saturating_sub(last) >= SNAPSHOT_REFRESH_SECS);

            WidgetDecision {
                node_id,
                target,
                final_capture,
                refresh_snapshot,
            }
        })
        .collect()
}

/// Гистерезисный порог live: живущие остаются до 0.23, новые — с 0.27.
fn live_eligible(w: &WidgetLodInput) -> bool {
    if !w.visible || w.overlaid {
        return false;
    }
    let threshold = if w.was_live {
        LIVE_EXIT_ZOOM
    } else {
        LIVE_ENTER_ZOOM
    };
    w.zoom >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: &str, last_seen: u64) -> WidgetLodInput {
        WidgetLodInput {
            node_id: id.to_owned(),
            visible: true,
            overlaid: false,
            package_ok: true,
            runtime_ok: true,
            has_instance: false,
            was_live: false,
            zoom: 1.0,
            last_seen,
        }
    }

    fn decisions(plan: &[WidgetDecision]) -> HashMap<&str, &WidgetDecision> {
        plan.iter().map(|d| (d.node_id.as_str(), d)).collect()
    }

    fn by_id<'a>(map: &'a HashMap<&str, &'a WidgetDecision>, id: &str) -> &'a WidgetDecision {
        map.get(id).copied().expect("решение есть")
    }

    #[test]
    fn six_visible_widgets_all_live() {
        let inputs: Vec<_> = (0..6).map(|i| input(&format!("w{i}"), i)).collect();
        let plan = plan_frame(&inputs, 100, &HashMap::new());
        assert!(plan.iter().all(|d| d.target == Target::Live), "{plan:?}");
    }

    #[test]
    fn ten_widgets_limit_and_refresh() {
        // 10 видимых при zoom 1.0: топ-6 по LRU live, 4 suspended c refresh
        let inputs: Vec<_> = (0..10).map(|i| input(&format!("w{i}"), i)).collect();
        let plan = plan_frame(&inputs, 100, &HashMap::new());
        let map = decisions(&plan);
        for i in 0..10 {
            let id = format!("w{i}");
            // last_seen = i: старшие значения свежее → w9..w4 live (топ-6), w3..w0 — пул
            let d = by_id(&map, &id);
            if i >= 4 {
                assert_eq!(d.target, Target::Live, "{id}");
            } else {
                assert_eq!(d.target, Target::Suspended, "{id}: {d:?}");
                // last_capture пуст → refresh положен сразу
                assert!(d.refresh_snapshot, "{id}");
            }
        }
    }

    #[test]
    fn refresh_respects_period_and_zoom() {
        // 7 видимых при zoom 1.0: седьмой (старейший) — за live-лимитом,
        // suspended; только ему положен периодический refresh
        let make = |i: u64| input(&format!("w{i}"), i);
        let inputs: Vec<WidgetLodInput> = (0..6).map(make).collect();
        let mut seventh = make(0);
        seventh.node_id = "seventh".to_owned();
        seventh.last_seen = 0;
        let mut inputs = inputs;
        inputs.push(seventh.clone());

        let mut last = HashMap::new();
        last.insert("seventh".to_owned(), 98);
        // 100 - 98 = 2 c < 5 c → refresh ещё не положен
        let plan = plan_frame(&inputs, 100, &last);
        let seventh_decision = plan
            .iter()
            .find(|d| d.node_id == "seventh")
            .expect("решение");
        assert_eq!(seventh_decision.target, Target::Suspended);
        assert!(!seventh_decision.refresh_snapshot, "период 5 с не истёк");
        // 104 - 98 ≥ 5 → refresh положен
        let plan = plan_frame(&inputs, 104, &last);
        let seventh_decision = plan
            .iter()
            .find(|d| d.node_id == "seventh")
            .expect("решение");
        assert!(seventh_decision.refresh_snapshot, "пора освежить");
        // При zoom < 0.25 refresh не нужен (мелко на экране), suspended остаётся
        let low_zoom: Vec<WidgetLodInput> = inputs
            .iter()
            .map(|w| {
                let mut w = w.clone();
                w.zoom = 0.2;
                w
            })
            .collect();
        let plan = plan_frame(&low_zoom, 104, &last);
        let seventh_decision = plan
            .iter()
            .find(|d| d.node_id == "seventh")
            .expect("решение");
        assert_eq!(seventh_decision.target, Target::Suspended);
        assert!(
            !seventh_decision.refresh_snapshot,
            "zoom < 0.25 — refresh не нужен"
        );
    }

    #[test]
    fn never_captured_widget_refreshes_immediately() {
        // CR-005: виджет без единого снапшота не ждёт SNAPSHOT_REFRESH_SECS
        // от старта приложения — первый захват назначается в первый кадр,
        // когда он suspended+видим. Виджет с уже снятым снапшотом живёт
        // по обычному периоду. Зум 0.26 — оба вне live (порог входа 0.27).
        let mut never = input("never", 1);
        never.zoom = 0.26;
        let mut captured = input("captured", 1);
        captured.zoom = 0.26;
        captured.has_instance = true;
        let mut last = HashMap::new();
        last.insert("captured".to_owned(), 98);
        // tick 100: снапшоту «captured» всего 2 с — refresh не положен;
        // «never» ключа не имеет — refresh назначен немедленно
        let plan = plan_frame(&[never.clone(), captured.clone()], 100, &last);
        let map = decisions(&plan);
        assert_eq!(by_id(&map, "never").target, Target::Suspended);
        assert_eq!(by_id(&map, "captured").target, Target::Suspended);
        assert!(
            by_id(&map, "never").refresh_snapshot,
            "без снапшота — захват сразу"
        );
        assert!(
            !by_id(&map, "captured").refresh_snapshot,
            "период 5 с не истёк"
        );
        // Снапшот «never» снят: дальше живёт по периоду
        let mut last = last;
        last.insert("never".to_owned(), 100);
        let plan = plan_frame(&[never, captured], 101, &last);
        let map = decisions(&plan);
        assert!(!by_id(&map, "never").refresh_snapshot, "свежий снапшот");
    }

    #[test]
    fn hysteresis_thresholds() {
        // Не был live: 0.26 < 0.27 — не входит
        let mut w = input("w", 1);
        w.zoom = 0.26;
        let plan = plan_frame(&[w.clone()], 0, &HashMap::new());
        assert_ne!(plan[0].target, Target::Live);
        // 0.27 — входит
        w.zoom = 0.27;
        let plan = plan_frame(&[w.clone()], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Live);
        // Был live: 0.25 ≥ 0.23 — остаётся
        w.zoom = 0.25;
        w.was_live = true;
        w.has_instance = true;
        let plan = plan_frame(&[w.clone()], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Live);
        // 0.22 < 0.23 — уходит в suspended с финальным снимком
        w.zoom = 0.22;
        let plan = plan_frame(&[w], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Suspended);
        assert!(plan[0].final_capture);
    }

    #[test]
    fn overlaid_and_invisible_fall_to_suspended() {
        let mut w = input("w", 1);
        w.was_live = true;
        w.has_instance = true;
        w.overlaid = true;
        let plan = plan_frame(&[w.clone()], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Suspended, "перекрытие — не live");
        assert!(plan[0].final_capture);
        assert!(!plan[0].refresh_snapshot, "перекрыт — refresh бессмыслен");

        let mut w = input("w2", 1);
        w.was_live = true;
        w.has_instance = true;
        w.visible = false;
        let plan = plan_frame(&[w], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Suspended);
    }

    #[test]
    fn broken_package_placeholder_and_destroy() {
        let mut broken = input("b", 5);
        broken.package_ok = false;
        let plan = plan_frame(&[broken.clone()], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Placeholder);
        assert_eq!(plan[0].render_mode(), RenderMode::Broken);

        broken.has_instance = true;
        broken.was_live = true;
        let plan = plan_frame(&[broken], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::PlaceholderDestroy);
        assert!(plan[0].final_capture, "уходя — снимок на память");

        let mut no_runtime = input("r", 5);
        no_runtime.runtime_ok = false;
        let plan = plan_frame(&[no_runtime], 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Placeholder);
    }

    #[test]
    fn beyond_pool_destroyed() {
        // 12 видимых: держатели — топ-10 по LRU (w11..w2), w0..w1 → Destroy
        let inputs: Vec<_> = (0..12).map(|i| input(&format!("w{i}"), i)).collect();
        let plan = plan_frame(&inputs, 0, &HashMap::new());
        let map = decisions(&plan);
        assert_eq!(by_id(&map, "w0").target, Target::Destroy);
        assert_eq!(by_id(&map, "w1").target, Target::Destroy);
        assert_ne!(by_id(&map, "w2").target, Target::Destroy);
        // Live — топ-6: w11..w6
        for i in 6..=11 {
            assert_eq!(by_id(&map, &format!("w{i}")).target, Target::Live);
        }
        // w2..w5 — suspended-пул
        for i in 2..=5 {
            assert_eq!(by_id(&map, &format!("w{i}")).target, Target::Suspended);
        }
    }

    #[test]
    fn no_instance_far_zoom_is_noinstance_not_destroy() {
        // Виджет без инстанса при zoom 0.1: инстанс не создаём, но и не «битый»
        let mut w = input("w", 1);
        w.zoom = 0.1;
        let plan = plan_frame(&[w], 0, &HashMap::new());
        // Держатель (внутри пула) — suspended; инстанса нет, host ничего не создаёт,
        // рендер рисует заглушку, Decision.target == Suspended остаётся валидным
        assert_eq!(plan[0].target, Target::Suspended);
        assert_eq!(plan[0].render_mode(), RenderMode::Snapshot);
        assert!(!plan[0].final_capture);
        assert!(!plan[0].refresh_snapshot);
    }

    #[test]
    fn lru_freshness_preferred() {
        // Равный зум, все видимы; свежесть last_seen решает, кто live
        let a = input("a", 100);
        let b = input("b", 200);
        // 7 виджетов: a старее — должно уйти в pool при споре
        let mut others: Vec<WidgetLodInput> =
            (0..5).map(|i| input(&format!("o{i}"), 300 + i)).collect();
        others.push(a.clone());
        others.push(b.clone());
        let plan = plan_frame(&others, 0, &HashMap::new());
        let map = decisions(&plan);
        assert_eq!(
            by_id(&map, "a").target,
            Target::Suspended,
            "старейший — не live"
        );
        assert_eq!(by_id(&map, "b").target, Target::Live);
    }
}
