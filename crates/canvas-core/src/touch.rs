//! FR-092 (мобильный web): машина жеста касания — чистая логика
//! распределения пальцев по жестам, без winit/App-зависимостей
//! (нативные тесты CI + wasip1-гейт; потребители — canvas-app на wasm).
//!
//! Проблема: winit-web для pointerType=touch шлёт ТОЛЬКО
//! `WindowEvent::Touch` (совместимые mouse-события подавлены
//! prevent_default — pointer.rs winit 0.30), а canvas-app обрабатывал
//! только мышь/клавиатуру — web-сборка на телефоне отрисовывалась, но
//! не отвечала на тап/драг/пинч («работает только HTML-оверлей»).
//!
//! Модель жеста (механика стандартных canvas-приложений, Miro/Figma):
//! - первый палец ведёт одиночный жест: тап/драг → левая кнопка мыши;
//! - второй палец ОТМЕНЯЕТ одиночный жест без коммита (драг ноды и
//!   резиновый прямоугольник не оставляются) и переводит пару в
//!   двухпальцевый режим: пан серединой + pinch-зум отношением
//!   дистанции (якорь — середина);
//! - любой палец вверх в двухпальцевом режиме завершает жест целиком —
//!   оставшийся палец не подхватывается (защита от ложного драга);
//! - `Cancelled` (pointercancel — касание забрал браузер) отменяет
//!   одиночный жест без коммита, машина остаётся консистентной.

/// Фаза касания (зеркало `winit::event::TouchPhase` — нейтральный тип
/// ядра: canvas-core не зависит от winit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Палец коснулся поверхности.
    Started,
    /// Палец переместился.
    Moved,
    /// Палец оторвался.
    Ended,
    /// Касание прервано системой (pointercancel).
    Cancelled,
}

/// Действие машины жеста — платформенный слой применяет его к мышино
/// му пайплайну/камере. Позиции — физические px (как winit
/// Touch.location); `pan`/`zoom_anchor` — логические px (пересчёт на
/// масштабе, переданном в [`TouchGesture::step`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Игнор (палец вне жеста: третий+ палец, чужой Move/End, база пинча).
    None,
    /// Касание начато: курсор в точку + нажать левую кнопку мыши.
    Press([f64; 2]),
    /// Перемещение одиночного жеста: курсор в точку.
    Move([f64; 2]),
    /// Касание завершено: курсор в точку + отпустить левую кнопку
    /// (позиция — последняя известная, палец мог не двигаться).
    Release([f64; 2]),
    /// Жест прерван без коммита (второй палец / pointercancel): снять
    /// транзиенты (драг ноды, резиновый прямоугольник и пр.).
    Cancel,
    /// Два пальца: пан серединой (лог. px) + зум отношением дистанции
    /// (якорь — середина, лог. px; фактор клампнут [0.5; 2.0] — защита
    /// от телепорта камеры при потере/возврате касания).
    TwoFinger {
        pan: [f32; 2],
        zoom_anchor: [f32; 2],
        zoom_factor: f32,
    },
}

/// Машина жеста касания (один экземпляр на окно/приложение).
#[derive(Debug, Default, Clone)]
pub struct TouchGesture {
    /// Активные пальцы: (идентификатор, последняя физ. позиция). Кап 2.
    active: Vec<(u64, [f64; 2])>,
    /// Палец, ведущий одиночный жест; None — одиночного жеста нет.
    single: Option<u64>,
    /// База двухпальцевого жеста: (середина, дистанция), физ. px.
    /// None — база ещё не снята (первый Moved пары только фиксирует).
    pinch: Option<([f64; 2], f64)>,
}

impl TouchGesture {
    pub fn new() -> Self {
        Self::default()
    }

    /// Обработать событие пальца; `scale` — масштаб (физ./лог.) для
    /// пересчёта pan/якоря зума (нестрогий: ≤ 0 заменяется на 1.0).
    pub fn step(&mut self, id: u64, phase: Phase, pos: [f64; 2], scale: f64) -> Action {
        match phase {
            Phase::Started => {
                if self.active.len() < 2 {
                    self.active.push((id, pos));
                }
                if self.active.len() == 1 {
                    if self.single.is_none() {
                        self.single = Some(id);
                        return Action::Press(pos);
                    }
                } else if self.single.take().is_some() {
                    // Второй палец: одиночный жест отменяется (не коммитится)
                    return Action::Cancel;
                }
                Action::None
            }
            Phase::Moved => {
                if let Some(slot) = self.active.iter_mut().find(|(fid, _)| *fid == id) {
                    slot.1 = pos;
                }
                if self.active.len() >= 2 && self.single.is_none() {
                    return self.two_finger(scale);
                }
                if self.single == Some(id) {
                    Action::Move(pos)
                } else {
                    Action::None
                }
            }
            Phase::Ended => {
                let was_single = self.single == Some(id);
                self.single = None;
                self.active.retain(|(fid, _)| *fid != id);
                if self.active.len() < 2 {
                    self.pinch = None;
                }
                if was_single {
                    Action::Release(pos)
                } else {
                    Action::None
                }
            }
            Phase::Cancelled => {
                let was_single = self.single == Some(id);
                self.single = None;
                self.active.retain(|(fid, _)| *fid != id);
                if self.active.len() < 2 {
                    self.pinch = None;
                }
                if was_single {
                    Action::Cancel
                } else {
                    Action::None
                }
            }
        }
    }

    /// Двухпальцевый режим: дельта середины → пан, отношение дистанций →
    /// зум. Первый Moved пары фиксирует базу без действия (иначе первый
    /// кадр давал бы телепорт камеры на всю накопленную дистанцию).
    fn two_finger(&mut self, scale: f64) -> Action {
        let (Some((_, a)), Some((_, b))) =
            (self.active.first().copied(), self.active.get(1).copied())
        else {
            return Action::None;
        };
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let dist = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        let Some((prev_mid, prev_dist)) = self.pinch else {
            self.pinch = Some((mid, dist));
            return Action::None;
        };
        if dist <= f64::EPSILON || prev_dist <= f64::EPSILON {
            // Выродившаяся геометрия (пальцы наложились): базу обновляем
            // только по валидной дистанции — после расхождения жест
            // продолжается с корректным фактором, без телепорта камеры.
            if dist > f64::EPSILON {
                self.pinch = Some((mid, dist));
            }
            return Action::None;
        }
        self.pinch = Some((mid, dist));
        let s = if scale > f64::EPSILON { scale } else { 1.0 };
        Action::TwoFinger {
            pan: [
                ((mid[0] - prev_mid[0]) / s) as f32,
                ((mid[1] - prev_mid[1]) / s) as f32,
            ],
            zoom_anchor: [(mid[0] / s) as f32, (mid[1] / s) as f32],
            zoom_factor: ((dist / prev_dist) as f32).clamp(0.5, 2.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Phase, TouchGesture};

    const S: f64 = 2.0; // масштаб физ./лог. для тестов

    fn tap(g: &mut TouchGesture, id: u64, x: f64, y: f64) -> Vec<Action> {
        vec![
            g.step(id, Phase::Started, [x, y], S),
            g.step(id, Phase::Ended, [x, y], S),
        ]
    }

    /// Тап: Press по позиции касания, Release по той же.
    #[test]
    fn tap_press_then_release() {
        let mut g = TouchGesture::new();
        let actions = tap(&mut g, 1, 100.0, 50.0);
        assert_eq!(actions[0], Action::Press([100.0, 50.0]));
        assert_eq!(actions[1], Action::Release([100.0, 50.0]));
    }

    /// Драг одиночным пальцем: Press → серия Move → Release в конечной
    /// точке (палец мог не двигаться между Moved и Ended — Release несёт
    /// последнюю позицию).
    #[test]
    fn drag_moves_then_release_at_last_position() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(7, Phase::Started, [10.0, 10.0], S),
            Action::Press([10.0, 10.0])
        );
        assert_eq!(
            g.step(7, Phase::Moved, [30.0, 40.0], S),
            Action::Move([30.0, 40.0])
        );
        assert_eq!(
            g.step(7, Phase::Ended, [45.0, 55.0], S),
            Action::Release([45.0, 55.0])
        );
    }

    /// Второй палец отменяет одиночный жест БЕЗ коммита (Cancel, не
    /// Release) — драг ноды/резиновый прямоугольник не оставляются;
    /// дальше пара шлёт TwoFinger, а не Move одиночки.
    #[test]
    fn second_finger_cancels_single_gesture() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(1, Phase::Started, [0.0, 0.0], S),
            Action::Press([0.0, 0.0])
        );
        assert_eq!(g.step(2, Phase::Started, [100.0, 0.0], S), Action::Cancel);
        // Первый Moved пары — база (без действия)
        assert_eq!(g.step(2, Phase::Moved, [110.0, 0.0], S), Action::None);
        let action = g.step(1, Phase::Moved, [10.0, 0.0], S);
        match action {
            Action::TwoFinger {
                pan,
                zoom_anchor,
                zoom_factor,
            } => {
                // Середина сдвинулась с (50,0) на (60,0): +5 физ. —
                // +2.5 лог.px на масштабе 2; дистанция 110 → 100
                assert!((pan[0] - 2.5).abs() < 1e-4, "пан: {pan:?}");
                assert_eq!(pan[1], 0.0);
                assert_eq!(zoom_anchor, [30.0, 0.0]); // середина 60 физ. / масштаб 2
                assert!(
                    (zoom_factor - 100.0 / 110.0).abs() < 1e-4,
                    "фактор: {zoom_factor}"
                );
            }
            other => panic!("ожидали TwoFinger, получено {other:?}"),
        }
    }

    /// Pinch: разведение пальцев ×2 — зум-фактор 2.0, якорь в середине,
    /// пан — по дельте середины (здесь — ноль).
    #[test]
    fn pinch_zoom_factor_from_distance_ratio() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(1, Phase::Started, [0.0, 0.0], S),
            Action::Press([0.0, 0.0])
        );
        assert_eq!(g.step(2, Phase::Started, [100.0, 0.0], S), Action::Cancel);
        // Первый Moved — база (середина (50,0), дистанция 100)
        assert_eq!(g.step(1, Phase::Moved, [0.0, 0.0], S), Action::None);
        // Разведение до 200 px (второй палец, первый неподвижен):
        // фактор 2.0; середина сместилась с (50,0) на (100,0) —
        // пан +25 лог.px (50 физ. на масштабе 2)
        let action = g.step(2, Phase::Moved, [200.0, 0.0], S);
        match action {
            Action::TwoFinger {
                pan,
                zoom_anchor,
                zoom_factor,
            } => {
                assert!((pan[0] - 25.0).abs() < 1e-4, "пан: {pan:?}");
                assert_eq!(pan[1], 0.0);
                assert_eq!(zoom_anchor, [50.0, 0.0]);
                assert!((zoom_factor - 2.0).abs() < 1e-4, "фактор: {zoom_factor}");
            }
            other => panic!("ожидали TwoFinger, получено {other:?}"),
        }
    }

    /// Любой палец вверх в двухпальцевом режиме завершает жест: у
    /// оставшегося пальца нет ни Move, ни авто-подхвата (защита от
    /// ложного драга после пинча); после полного отпускания новый палец
    /// начинает одиночный жест.
    #[test]
    fn two_finger_gesture_ends_on_any_lift() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(1, Phase::Started, [0.0, 0.0], S),
            Action::Press([0.0, 0.0])
        );
        assert_eq!(g.step(2, Phase::Started, [100.0, 0.0], S), Action::Cancel);
        assert_eq!(g.step(2, Phase::Ended, [100.0, 0.0], S), Action::None);
        // Оставшийся палец двигается — действие None (не подхватывается)
        assert_eq!(g.step(1, Phase::Moved, [50.0, 0.0], S), Action::None);
        // И его поднятие ничего не коммитит
        assert_eq!(g.step(1, Phase::Ended, [50.0, 0.0], S), Action::None);
        // Новый палец начинает одиночный жест
        assert_eq!(
            g.step(3, Phase::Started, [10.0, 10.0], S),
            Action::Press([10.0, 10.0])
        );
    }

    /// Третий палец игнорируется (кап 2) и не отменяет двухпальцевый жест.
    #[test]
    fn third_finger_is_ignored() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(1, Phase::Started, [0.0, 0.0], S),
            Action::Press([0.0, 0.0])
        );
        assert_eq!(g.step(2, Phase::Started, [100.0, 0.0], S), Action::Cancel);
        assert_eq!(g.step(1, Phase::Moved, [0.0, 0.0], S), Action::None);
        // Третий палец: Started при полном окне — None, пара живёт
        assert_eq!(g.step(3, Phase::Started, [200.0, 200.0], S), Action::None);
        let action = g.step(2, Phase::Moved, [110.0, 0.0], S);
        assert!(matches!(action, Action::TwoFinger { .. }));
    }

    /// Pointercancel одиночного жеста — Cancel (не Release): касание
    /// забрал браузер, коммитить клик/драг нельзя; машина чистая —
    /// следующий палец начинает новый жест.
    #[test]
    fn cancel_phase_cancels_without_commit() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(1, Phase::Started, [5.0, 5.0], S),
            Action::Press([5.0, 5.0])
        );
        assert_eq!(g.step(1, Phase::Cancelled, [5.0, 5.0], S), Action::Cancel);
        assert_eq!(
            g.step(2, Phase::Started, [7.0, 7.0], S),
            Action::Press([7.0, 7.0])
        );
    }

    /// Нулевая дистанция пальцев (наложение) — деление защищено: без
    /// паники, действие None до восстановления дистанции.
    #[test]
    fn zero_distance_is_guarded() {
        let mut g = TouchGesture::new();
        assert_eq!(
            g.step(1, Phase::Started, [10.0, 10.0], S),
            Action::Press([10.0, 10.0])
        );
        assert_eq!(g.step(2, Phase::Started, [20.0, 10.0], S), Action::Cancel);
        assert_eq!(g.step(1, Phase::Moved, [10.0, 10.0], S), Action::None);
        assert_eq!(g.step(2, Phase::Moved, [10.0, 10.0], S), Action::None);
        // Дистанция снова ненулевая — жест работает с корректной базой
        let action = g.step(2, Phase::Moved, [30.0, 10.0], S);
        match action {
            Action::TwoFinger {
                pan, zoom_factor, ..
            } => {
                // База — последняя валидная (15,10) с дистанцией 10:
                // середина (15,10) → (20,10), фактор 20/10 = 2.0
                assert!((pan[0] - 2.5).abs() < 1e-4, "пан: {pan:?}");
                assert!((zoom_factor - 2.0).abs() < 1e-4, "фактор: {zoom_factor}");
            }
            other => panic!("ожидали TwoFinger, получено {other:?}"),
        }
    }
}
