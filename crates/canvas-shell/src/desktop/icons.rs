//! Скрытие системных иконок десктопа + sentinel-краш-сейф (T17-A,
//! RECIPES R5/R9, SPEC §7.4 п.5).
//!
//! R5 (идемпотентность): `WM_COMMAND 0x7402` на SHELLDLL_DefView —
//! ПЕРЕКЛЮЧАТЕЛЬ, а не установка. Перед отправкой читаем фактическое
//! состояние `SHGetSetSettings(SSF_HIDEICONS)` и шлём toggle только при
//! расхождении с желаемым — иначе повторный запуск ВКЛЮЧИТ иконки
//! вместо выключения. Запись через `fSet=TRUE` в Win10+ не работает —
//! не пытаться (проверено Lively). Исходное состояние фиксируем в
//! [`IconGuard`] и восстанавливаем при штатном выходе.
//!
//! Краш-сейф: если мы скрыли иконки, создаём sentinel-файл в
//! `default_cache_dir()`; kill -9 обходит Drop-страховку — следующий
//! запуск (любой режим, [`crash_recovery`]) видит sentinel и
//! форс-восстанавливает иконки.
//!
//! R9: рефреш десктопа — ТОЛЬКО `InvalidateRect+UpdateWindow(DefView)`;
//! `SPI_SETDESKWALLPAPER` под ЗАПРЕТОМ (на raised desktop разрушает
//! WorkerW, SPEC §7.4 / RECIPES R9).
//!
//! Файл смешанный (рекомендация T15-A): чистые решения/протокол
//! sentinel — кроссплатформенны (тесты на Linux); Win32-механика —
//! cfg(windows)-блоки. Зона воркера T17-A: реализация по плану
//! docs/plans/T17-desktop-polish.md §3 (icons.rs) — публичные
//! сигнатуры заморожены координатором.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Бит `fHideIcons` в `_bitfield1` SHELLSTATEA (ShlObj.h, бит 7):
/// чистое декодирование состояния из битового поля (packed-структуру
/// читаем по значению — ссылки на поля packed — UB, план §7).
pub const HIDE_ICONS_BIT: i32 = 0x80;

/// Команда-toggle иконок на SHELLDLL_DefView (Lively DesktopUtil.cs,
/// RECIPES R5): WM_COMMAND с этим параметром переключает видимость
/// иконок десктопа.
pub const TOGGLE_ICONS_COMMAND: usize = 0x7402;

/// Имя sentinel-файла в каталоге приложения: логика краш-сейва читает
/// только факт существования (содержимое — диагностический штамп,
/// план §3).
const SENTINEL_FILE_NAME: &str = "desktop-icons.sentinel";

/// Декодировать fHideIcons из `_bitfield1` SHELLSTATEA (бит 7).
pub fn hide_icons_state(bitfield1: i32) -> bool {
    // Чистая маска: посторонние биты поля не влияют (значим только бит 7).
    (bitfield1 & HIDE_ICONS_BIT) != 0
}

/// Нужно ли слать toggle (R5): только при расхождении фактического
/// состояния с желаемым (XOR).
pub fn should_toggle(hidden_now: bool, want_hidden: bool) -> bool {
    // XOR: при совпадении факт/цель команда не отправляется — повторный
    // запуск не «мигает» иконками (идемпотентность R5).
    hidden_now != want_hidden
}

/// Путь sentinel-файла краш-сейва в каталоге приложения (каталог —
/// `canvas_shell::default_cache_dir()`, единый источник T6/T14).
pub fn sentinel_path(dir: &Path) -> PathBuf {
    dir.join(SENTINEL_FILE_NAME)
}

/// Sentinel существует — прошлая сессия умерла, не восстановив иконки.
pub fn sentinel_exists(dir: &Path) -> bool {
    // exists() покрывает и «каталога нет» (false) — чистый старт.
    sentinel_path(dir).exists()
}

/// Создать sentinel (содержимое — UTC-штамп для диагностики; логика
/// читает только факт существования). Ошибка записи — деградация:
/// краш-сейф не сработает, но работу не рушит (R14).
pub fn sentinel_create(dir: &Path) {
    // Каталог — идемпотентно: default_cache_dir может отсутствовать на
    // первом запуске; create_dir_all на существующем каталоге — no-op.
    if let Err(err) = std::fs::create_dir_all(dir) {
        tracing::warn!(
            %err,
            ?dir,
            "icons: sentinel — каталог не создан (краш-сейф деградирован, R14)"
        );
        return;
    }
    // UTC-штамп (unix-секунды) — только для диагностики крашей по логам
    // владельца; логикой не парсится (план §3).
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_secs())
        // часы раньше эпохи — на практике невозможно; 0 честнее паники
        .unwrap_or(0);
    let path = sentinel_path(dir);
    if let Err(err) = std::fs::write(&path, format!("canvasdesk-icons-hidden unix-utc={stamp}\n")) {
        tracing::warn!(
            %err,
            ?path,
            "icons: sentinel не записан (краш-сейф деградирован, R14)"
        );
    }
}

/// Удалить sentinel (штатное восстановление). Отсутствие файла — не
/// ошибка (идемпотентность).
pub fn sentinel_remove(dir: &Path) {
    let path = sentinel_path(dir);
    match std::fs::remove_file(&path) {
        Ok(()) => tracing::debug!(?path, "icons: sentinel снят"),
        // Отсутствие файла — идемпотентность, не ошибка
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        // Прочие ошибки (права/блокировка) — не рушат работу: sentinel
        // останется до следующего снятия, а crash_recovery безвреден при
        // показанных иконках — уровень debug («молча», план §3).
        Err(err) => tracing::debug!(%err, ?path, "icons: sentinel не удалён"),
    }
}

#[cfg(windows)]
use super::hierarchy::{ensure_worker_w, find_progman};
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
#[cfg(windows)]
use windows::Win32::UI::Shell::{SHGetSetSettings, SHELLSTATEA, SSF_HIDEICONS};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_COMMAND};

/// Каталог sentinel-файла — `~/.canvasdesk` (default_cache_dir
/// приложения, план §8.5: конфиг-каталог гарантированно записываем).
/// Формула выбора корня (USERPROFILE|HOME → `.canvasdesk`) — та же, что
/// в `canvas_shell::default_cache_dir()` (единый источник T6/T14);
/// прямой вызов невозможен: зонд win-check включает `desktop/`
/// #[path]-включением без lib.rs/cache.rs, пути `crate::` там нет —
/// потому локальная копия формулы (не значения). Дрейф безопасен:
/// sentinel одноразовый, при переезде каталога в T19 НЕ мигрирует
/// (план §8.5).
#[cfg(windows)]
fn sentinel_cache_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".canvasdesk"))
}

/// Таймаут toggle-команды DefView (мс): зависший DefView не должен
/// блокировать поток приложения дольше пары секунд (R5/R14).
#[cfg(windows)]
const TOGGLE_TIMEOUT_MS: u32 = 2000;

/// Владелец скрытия иконок (R5): помнит исходное состояние, шлёт
/// toggle только при расхождении, восстанавливает идемпотентно.
/// Drop-страховка от unwind (паники); kill -9 покрыт sentinel'ом.
#[cfg(windows)]
pub struct IconGuard {
    /// DefView-слой иконок (из DesktopHierarchy, T15).
    def_view: HWND,
    /// Иконки скрыли МЫ (исходное состояние = показаны): только тогда
    /// restore трогает систему. Пользовательское «скрыто» не наше.
    hidden_by_us: bool,
    /// Каталог sentinel-файла (default_cache_dir приложения).
    sentinel_dir: Option<PathBuf>,
}

#[cfg(windows)]
impl IconGuard {
    /// Зафиксировать исходное состояние иконок (до скрытия). Отказ
    /// чтения SHGetSetSettings — warn + guard в no-op-режиме (R14).
    pub fn capture(def_view: HWND) -> Self {
        // Факт исходного состояния не храним полем (каждый toggle
        // перечитывает SHELLSTATE заново, R5) — нужен только лог для
        // приёмки §9 п.1 и sentinel-каталог.
        match read_hide_icons() {
            Some(hidden) => {
                tracing::info!(hidden, "icons: исходное состояние fHideIcons");
            }
            None => {
                // Чтение отказало: hide/show будут no-op (R14), sentinel
                // никогда не создастся — деградация безопасна.
                tracing::warn!("icons: исходное состояние нечитаемо — guard в no-op (R14)");
            }
        }
        Self {
            def_view,
            hidden_by_us: false,
            sentinel_dir: sentinel_cache_dir(),
        }
    }

    /// Скрыть иконки (идемпотентно; R5 — toggle только при
    /// расхождении; sentinel создаём только если реально тогглили).
    pub fn hide(&mut self) {
        // ФАКТ перечитываем перед каждым toggle (R5); цель — скрыто.
        let Some(hidden_now) = read_hide_icons() else {
            // Состояние нечитаемо: слать toggle вслепую нельзя (R5 —
            // команда переключает: можно ВКЛЮЧИТЬ иконки вместо
            // выключения) → no-op (R14).
            tracing::warn!("icons: hide пропущен — fHideIcons нечитаем (R14)");
            return;
        };
        if should_toggle(hidden_now, true) {
            toggle_icons(self.def_view);
            refresh_desktop(self.def_view);
            // Sentinel только при реальном toggle — чистый выход снимет
            // (план §5: «sentinel создан ТОЛЬКО когда реально тогглили»).
            if let Some(dir) = self.sentinel_dir.as_deref() {
                sentinel_create(dir);
            }
            self.hidden_by_us = true;
            tracing::info!("icons: системные иконки скрыты нами");
        } else {
            // Уже скрыты (юзером или нашим повторным hide) — состояние
            // не наше менять, не трогаем (R5).
            tracing::debug!("icons: иконки уже скрыты — hide no-op");
        }
    }

    /// Показать иконки (toggle-пункт меню; зеркально hide).
    pub fn show(&mut self) {
        // Зеркально hide (R5): факт перечитывается, цель — показаны.
        // Показываем независимо от того, кто скрывал: в --desktop ПКМ
        // Explorer перехвачен нашим канвасом — без этого юзер не смог бы
        // вернуть иконки (план §9 п.3).
        let Some(hidden_now) = read_hide_icons() else {
            tracing::warn!("icons: show пропущен — fHideIcons нечитаем (R14)");
            return;
        };
        if should_toggle(hidden_now, false) {
            toggle_icons(self.def_view);
            refresh_desktop(self.def_view);
            // Sentinel при показе снимается: краш-сейв «иконки скрыты»
            // неактуален (файл одноразовый, чей бы он ни был).
            if let Some(dir) = self.sentinel_dir.as_deref() {
                sentinel_remove(dir);
            }
            self.hidden_by_us = false;
            tracing::info!("icons: системные иконки показаны");
        } else {
            // Уже показаны — не трогаем (R5).
            tracing::debug!("icons: иконки уже показаны — show no-op");
        }
    }

    /// Восстановить исходное состояние (штатный выход): идемпотентно,
    /// sentinel снимается. Выхода из режима не делает — только иконки.
    pub fn restore(&mut self) {
        // Только наше вмешательство: юзерское «скрыто» — состояние
        // юзера, при выходе не трогаем (R5).
        if !self.hidden_by_us {
            return;
        }
        // «Как show», но вызывается безусловно при выходе/Drop — сама
        // механика идемпотентна. Факт перечитываем: юзер мог показать
        // иконки сам (Shell-путь, вне канваса) — безусловный toggle
        // СКРЫЛ бы иконки навсегда (R5).
        let Some(hidden_now) = read_hide_icons() else {
            // Состояние нечитаемо: sentinel НЕ снимаем — crash_recovery
            // следующего запуска дочистит; hidden_by_us сохраняем
            // (идемпотентность: Drop может повторить попытку).
            tracing::warn!("icons: restore — fHideIcons нечитаем, sentinel остаётся (R14)");
            return;
        };
        if should_toggle(hidden_now, false) {
            toggle_icons(self.def_view);
            refresh_desktop(self.def_view);
        }
        // Показали (или юзер уже показал сам) — краш-сейв снимается.
        if let Some(dir) = self.sentinel_dir.as_deref() {
            sentinel_remove(dir);
        }
        self.hidden_by_us = false;
        tracing::info!("icons: исходное состояние иконок восстановлено");
    }

    /// Иконки сейчас скрыты нами (для галочки пункта меню).
    pub fn hidden_by_us(&self) -> bool {
        self.hidden_by_us
    }

    /// Перенаправить на новый DefView после перезапуска Explorer /
    /// re-attach (координаторский glue T17-E): иерархия пересоздана —
    /// старый HWND мёртв, toggle ушёл бы в пустоту. `hidden_by_us`
    /// сохраняется: SHELLSTATE персистентен — новый DefView рисует по
    /// нему, повторный hide() идемпотентно до-скроет при расхождении.
    pub fn repoint(&mut self, def_view: HWND) {
        self.def_view = def_view;
    }
}

/// Drop-страховка от unwind: то же, что restore (паники в event loop);
/// kill -9 обходит Drop — на то sentinel (план §3).
#[cfg(windows)]
impl Drop for IconGuard {
    fn drop(&mut self) {
        // SAFETY (Drop-контракт): паника внутри drop при уже идущем
        // unwind = double-panic = abort процесса. restore спроектирован
        // без паник: все отказы (read/toggle/sentinel) — warn/debug;
        // worst case остаётся sentinel → crash_recovery (план §3).
        self.restore();
    }
}

/// Рефреш десктопа после toggle (R9): InvalidateRect(DefView, None,
/// false) + UpdateWindow(DefView). SPI_SETDESKWALLPAPER НЕ вызывать
/// никогда (raised-разрушение WorkerW, RECIPES R9).
#[cfg(windows)]
pub fn refresh_desktop(def_view: HWND) {
    // SAFETY: def_view — хэндл слоя иконок (детект T15-B или
    // crash_recovery); lprect=None — инвалидация всей клиентской области;
    // berase=false — без стирания фона (перерисовка минимальна);
    // BOOL-FALSE (в т.ч. висячий хэндл) — деградация warn, не UB.
    let invalidated = unsafe { InvalidateRect(Some(def_view), None, false) };
    if !invalidated.as_bool() {
        tracing::warn!("icons: InvalidateRect(DefView) провален (рефреш деградирован, R14)");
    }
    // SAFETY: def_view валиден, если InvalidateRect прошёл; при отказе
    // UpdateWindow на висячем окне безвреден (просто FALSE). Посылает
    // WM_PAINT напрямую, минуя очередь.
    let _ = unsafe { UpdateWindow(def_view) };
}

/// Краш-сейф при старте (TASKS T17): sentinel существует и иконки
/// сейчас скрыты → форс-восстановление (toggle + refresh + снять
/// sentinel) → true; sentinel без скрытых иконок → просто снять (юзер
/// показал сам) → false. Вызывается из main() ДО attach, без гейта
/// --desktop (план §8.6). Возвращает факт восстановления (для лога).
#[cfg(windows)]
pub fn crash_recovery() -> bool {
    let Some(cache_dir) = sentinel_cache_dir() else {
        // Нет домашнего каталога — sentinel неоткуда читать: краш-сейф
        // деградирован, приложение работает дальше (R14).
        tracing::warn!("icons: crash_recovery — домашний каталог недоступен (R14)");
        return false;
    };
    if !sentinel_exists(&cache_dir) {
        // Чистый старт: прошлые сессии снимали sentinel штатно.
        return false;
    }
    // Sentinel есть — прошлая сессия умерла со скрытыми иконками. Но
    // состояние могло измениться (юзер показал сам, новый Explorer
    // нарисовал по персистентному SSF_HIDEICONS) — сверяемся с фактом.
    let Some(hidden_now) = read_hide_icons() else {
        // Нечитаемо: sentinel НЕ снимаем — следующий запуск повторит
        // попытку (консервативность краш-сейва, R14).
        tracing::warn!("icons: crash_recovery — fHideIcons нечитаем, sentinel остаётся (R14)");
        return false;
    };
    if !hidden_now {
        // Иконки уже показаны — сигнал неактуален, снимаем молча.
        sentinel_remove(&cache_dir);
        tracing::debug!("icons: crash_recovery — иконки уже показаны, sentinel снят");
        return false;
    }
    // Иконки скрыты мёртвой сессией — форс-восстановление. DefView —
    // через детект иерархии (ensure_worker_w повторяет путь детекта
    // без 0x052C при живом WorkerW, R4).
    let def_view = match find_progman().and_then(ensure_worker_w) {
        Ok(hierarchy) => hierarchy.def_view,
        Err(err) => {
            // Иерархии нет (shell не Explorer, RDP, рестарт Explorer
            // прямо сейчас) — восстановить некому; sentinel остаётся на
            // следующий запуск (R14).
            tracing::warn!(
                ?err,
                "icons: crash_recovery — иерархия недоступна, sentinel остаётся"
            );
            return false;
        }
    };
    toggle_icons(def_view);
    refresh_desktop(def_view);
    sentinel_remove(&cache_dir);
    tracing::info!("icons: иконки восстановлены после краша (sentinel снят)");
    true
}

/// Прочитать фактическое состояние fHideIcons (SHELLSTATEA, R5):
/// источник истины перед КАЖДЫМ toggle. None — отказ чтения → вызывающие
/// переходят в no-op (деградация R14).
#[cfg(windows)]
fn read_hide_icons() -> Option<bool> {
    // Сверка маски с ShlObj.h: SSF_HIDEICONS = 16384 (ловит рассинхрон
    // при апгрейде windows-crate, идиома модулей T15/T16).
    debug_assert_eq!(SSF_HIDEICONS.0, 16384, "SSF_HIDEICONS разошёлся с ShlObj.h");
    // SHELLSTATEA — repr(C, packed(1)): структура копируется ПО ЗНАЧЕНИЮ
    // (Default); ссылок на ПОЛЯ не берём (unaligned — UB, план §7);
    // ссылка на всю структуру (выравнивание структуры = 1) валидна.
    let mut state = SHELLSTATEA::default();
    // SHGetSetSettings — void-FFI без кода ошибки: «отказа» в возврате
    // нет; аварийное исключение сквозь FFI ловим catch_unwind — контракт
    // read: отказ → warn + None (деградация R14).
    let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: state — корректная SHELLSTATEA::default() на стеке,
        // указатель валиден всё время синхронного вызова; dwmask =
        // SSF_HIDEICONS — заполняется только бит fHideIcons; bset=false
        // — режим ЧТЕНИЯ (запись fSet=TRUE в Win10+ не работает и не
        // используется, R5); вызов ничего не аллоцирует и не владеет.
        unsafe { SHGetSetSettings(Some(&mut state), SSF_HIDEICONS, false) };
    }));
    if read.is_err() {
        tracing::warn!("icons: SHGetSetSettings аварийно прерван — read → None (R14)");
        return None;
    }
    // Поле читаем копией i32 (packed: чтение по значению легально).
    Some(hide_icons_state(state._bitfield1))
}

/// Отправить toggle-команду 0x7402 слою иконок DefView (R5):
/// WM_COMMAND, WPARAM = команда, LPARAM = 0 (параметры рецепта Lively).
/// Приватная: вызывается ТОЛЬКО после read_hide_icons при расхождении
/// состояний (идемпотентность R5).
#[cfg(windows)]
fn toggle_icons(def_view: HWND) {
    // SAFETY: def_view — хэндл слоя иконок (детект иерархии T15-B /
    // ensure_worker_w в crash_recovery); висячий хэндл даёт отказ
    // SendMessageTimeoutW (LRESULT 0), не UB. WM_COMMAND с
    // WPARAM(0x7402)/LPARAM(0) — toggle-рецепт R5 (недокументированная
    // команда, Lively DesktopUtil); SMTO_ABORTIFHUNG + 2000 мс —
    // зависший DefView не блокирует поток приложения; lpdwresult=None —
    // результат команды не читаем; вызов синхронный, без владения.
    let delivered = unsafe {
        SendMessageTimeoutW(
            def_view,
            WM_COMMAND,
            WPARAM(TOGGLE_ICONS_COMMAND),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            TOGGLE_TIMEOUT_MS,
            None,
        )
    };
    if delivered.0 == 0 {
        // 0 = таймаут/отказ (hang или смерть DefView): иконки остались
        // в прежнем состоянии — sentinel решит краш-сейф следующего
        // запуска (R14).
        tracing::warn!("icons: toggle 0x7402 не доставлен (таймаут DefView, R14)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Уникальный temp-каталог теста (паттерн watcher.rs/search.rs:
    /// pid + атомарный счётчик): реальный ~/.canvasdesk не трогаем,
    /// параллельные прогоны не конфликтуют, мусор прошлого прогона
    /// счищается до создания.
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "canvasdesk-icons-{}-{}-{tag}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("создать tempdir");
        dir
    }

    // ---------- hide_icons_state ----------

    /// Таблица декодирования бита 7 (R5): 0x80 → скрыты; 0 и 0x7F (все
    /// биты кроме 7-го) → показаны; 0x80|1 (посторонние биты не мешают)
    /// и -1 (все биты) → скрыты.
    #[test]
    fn hide_icons_state_bit7_table() {
        assert!(hide_icons_state(0x80));
        assert!(!hide_icons_state(0));
        assert!(hide_icons_state(0x80 | 1));
        assert!(!hide_icons_state(0x7F));
        assert!(hide_icons_state(-1));
    }

    /// Константа бита — точное значение ShlObj.h (локальная копия
    /// модуля; cfg(windows)-код сверяет маску SSF по windows-crate).
    #[test]
    fn hide_icons_bit_constant_value() {
        assert_eq!(HIDE_ICONS_BIT, 0x80);
    }

    // ---------- should_toggle ----------

    /// XOR-таблица R5: toggle отправляется только при расхождении
    /// фактического состояния с желаемым — все 4 комбинации.
    #[test]
    fn should_toggle_xor_table() {
        assert!(should_toggle(true, false));
        assert!(should_toggle(false, true));
        assert!(!should_toggle(true, true));
        assert!(!should_toggle(false, false));
    }

    /// Команда toggle — точное значение рецепта R5 (Lively
    /// DesktopUtil: 0x7402): основа протокола, сверяем копию.
    #[test]
    fn toggle_icons_command_constant_value() {
        assert_eq!(TOGGLE_ICONS_COMMAND, 0x7402);
    }

    // ---------- sentinel-протокол ----------

    /// Путь: фиксированное имя файла в переданном каталоге (каталог —
    /// default_cache_dir приложения, план §8.5).
    #[test]
    fn sentinel_path_file_name() {
        let dir = Path::new("/tmp/canvasdesk");
        assert_eq!(
            sentinel_path(dir),
            PathBuf::from("/tmp/canvasdesk/desktop-icons.sentinel")
        );
    }

    /// Протокол краш-сейва: create → exists → remove → !exists;
    /// содержимое — непустой диагностический штамп (не пустой файл).
    #[test]
    fn sentinel_create_exists_remove_cycle() {
        let dir = temp_dir("cycle");
        assert!(!sentinel_exists(&dir));
        sentinel_create(&dir);
        assert!(sentinel_exists(&dir));
        let content = std::fs::read_to_string(sentinel_path(&dir)).expect("прочитать sentinel");
        assert!(!content.is_empty(), "диагностический штамп не записан");
        sentinel_remove(&dir);
        assert!(!sentinel_exists(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Идемпотентность снятия: повторный remove несуществующего
    /// sentinel не паникует и не считается ошибкой (план §3).
    #[test]
    fn sentinel_remove_missing_is_noop() {
        let dir = temp_dir("remove-missing");
        sentinel_remove(&dir); // файла нет — тихо
        sentinel_create(&dir);
        sentinel_remove(&dir);
        sentinel_remove(&dir); // уже снят — тихо
        assert!(!sentinel_exists(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// create в несуществующем подкаталоге создаёт его:
    /// default_cache_dir может отсутствовать на первом запуске.
    #[test]
    fn sentinel_create_makes_missing_dir() {
        let base = temp_dir("missing-dir");
        let dir = base.join("nested");
        sentinel_create(&dir);
        assert!(sentinel_exists(&dir));
        let _ = std::fs::remove_dir_all(&base);
    }
}
