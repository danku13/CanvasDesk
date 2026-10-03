//! FR-095/096/097 (мобильный web): атомарный мост состояния между
//! canvas-app (платформенно-нейтральный слой) и canvas-web (DOM-слой).
//!
//! Оба крейта и так зависят от canvas-core — мост через ядро не создаёт
//! циклов (canvas-app не может вызывать canvas-web напрямую: зависимость
//! направлена web → app). Атомики доступны под wasm32 без JS-рунтайма,
//! на нативе просто не используются (значения по умолчанию).
//!
//! Каналы:
//! - [`TOUCH_PRESS_GEN`] — будилка long-press: палец без движения событий
//!   касания не рождает, поэтому время для машины жеста
//!   ([`crate::touch::TouchGesture::poll_long_press`]) доставляет
//!   платформа. canvas-app инкрементирует счётчик на каждом
//!   `Action::Press`, canvas-web замечает смену и ставит
//!   `setTimeout(LONG_PRESS_MS)` → `AppEvent::LongPressPoll`.
//!   Relaxed-упорядоченность достаточна: латентность доставки — до тика
//!   опроса web-слоя, корректность решает машина (poll защищён от
//!   ложных срабатываний: палец поднят / ушёл за slop / уже выдан).
//! - [`POINTER_COARSE`] — coarse-указатель (тач — основной ввод);
//!   источник `matchMedia("(pointer: coarse)")` в canvas-web. На нативе
//!   false: тачскрины Windows идут через OS-эмуляцию мыши (FR-092),
//!   поведение натива не меняется.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// FR-096: поколение тач-нажатия (см. модульную доку).
pub static TOUCH_PRESS_GEN: AtomicU64 = AtomicU64::new(0);

/// FR-097: coarse-указатель — тач-экран основной ввод (см. модульную доку).
pub static POINTER_COARSE: AtomicBool = AtomicBool::new(false);

/// FR-096: зафиксировать новое тач-нажатие (вызывает canvas-app на
/// `Action::Press` машины жеста).
pub fn bump_touch_press_gen() {
    TOUCH_PRESS_GEN.fetch_add(1, Ordering::Relaxed);
}

/// FR-096: текущее поколение нажатия (читает canvas-web).
pub fn touch_press_gen() -> u64 {
    TOUCH_PRESS_GEN.load(Ordering::Relaxed)
}

/// FR-097: записать признак coarse-указателя (вызывает canvas-web).
pub fn set_pointer_coarse(coarse: bool) {
    POINTER_COARSE.store(coarse, Ordering::Relaxed);
}

/// FR-097: coarse-указатель активен? (читает canvas-app в hit-тестах).
pub fn pointer_coarse() -> bool {
    POINTER_COARSE.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Мост — атомики: bump/load согласованы, set/load coarse согласованы.
    /// Глобальное состояние — тесты нейтральны к порядку (инкремент
    /// относительно текущего значения, coarse — симметричная пара).
    #[test]
    fn gen_and_coarse_roundtrip() {
        let before = touch_press_gen();
        bump_touch_press_gen();
        assert_eq!(touch_press_gen(), before + 1);

        let was = pointer_coarse();
        set_pointer_coarse(true);
        assert!(pointer_coarse());
        set_pointer_coarse(was);
        assert_eq!(pointer_coarse(), was);
    }
}
