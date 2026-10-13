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
//!   поведение натива не меняется. Thread-local (W3 #73): писатель —
//!   главный поток web-рантайма, читатели — hit-тесты того же потока,
//!   поэтому изоляция по потокам не меняет продуктовую семантику, но
//!   устраняет межтестовую гонку параллельного libtest (flake macOS CI:
//!   переключения coarse видели непиненные hit-тесты palette/whatif_ui —
//!   upgrade-log §W3, решение 5).

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

/// FR-096: поколение тач-нажатия (см. модульную доку).
pub static TOUCH_PRESS_GEN: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// FR-097: coarse-указатель — тач-экран основной ввод (см. модульную
    /// доку; thread-local — W3 #73, гонка параллельных тестов).
    pub static POINTER_COARSE: Cell<bool> = const { Cell::new(false) };
}

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
    POINTER_COARSE.set(coarse);
}

/// FR-097: coarse-указатель активен? (читает canvas-app в hit-тестах).
pub fn pointer_coarse() -> bool {
    POINTER_COARSE.get()
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
