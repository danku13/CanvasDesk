//! Менеджер sidecar-процесса `laya-serve` (паттерн `canvasdesk mcp`).
//!
//! Ленивый spawn при первом L1-запросе → health-wait (GET /health до
//! готовности чекпойнта, cold start до 60 с) → idle-shutdown → shutdown
//! при выходе приложения. Паника/зависание sidecar не роняет хост:
//! клиент получает Transport-ошибку и деградирует на lex.

use std::io;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Конфигурация запуска sidecar.
#[derive(Debug, Clone)]
pub struct SidecarConfig {
    /// Команда запуска (путь к `laya-serve` или обёртке).
    pub command: String,
    /// Аргументы командной строки (порт/чекпойнт/head_max_len…).
    pub args: Vec<String>,
    /// Простой до выгрузки (конфиг `idle_shutdown_s`, дефолт 600 с).
    pub idle_shutdown: Duration,
}

impl SidecarConfig {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            idle_shutdown: Duration::from_secs(600),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_idle_shutdown(mut self, d: Duration) -> Self {
        self.idle_shutdown = d;
        self
    }
}

/// Запущенный sidecar-процесс. Владеет ребёнком; `Drop` завершает процесс.
#[derive(Debug)]
pub struct Sidecar {
    child: Option<Child>,
    spawned_at: Instant,
    last_used: Instant,
}

impl Sidecar {
    /// Запустить процесс (stdout/stderr — в null: логи sidecar не нужны
    /// хосту, диагностика — через health/smoke).
    pub fn spawn(cfg: &SidecarConfig) -> io::Result<Self> {
        let child = Command::new(&cfg.command)
            .args(&cfg.args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let now = Instant::now();
        Ok(Self {
            child: Some(child),
            spawned_at: now,
            last_used: now,
        })
    }

    /// Запущен ли ещё (неблокирующая проверка; зомби-процесс жнётся).
    pub fn is_running(&mut self) -> bool {
        match &mut self.child {
            None => false,
            Some(c) => matches!(c.try_wait(), Ok(None)),
        }
    }

    /// PID процесса (для логов/RSS-контроля).
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// Время жизни процесса (диагностика cold start в HUD/логах).
    pub fn uptime(&self) -> Duration {
        self.spawned_at.elapsed()
    }

    /// Отметить использование (после успешного запроса).
    pub fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    /// Длительность простоя с последнего использования.
    pub fn idle_for(&self) -> Duration {
        self.last_used.elapsed()
    }

    /// Выгрузить, если простой превысил лимит. true — если выгрузили.
    pub fn shutdown_if_idle(&mut self, limit: Duration) -> bool {
        if self.idle_for() >= limit {
            self.shutdown();
            true
        } else {
            false
        }
    }

    /// Завершить процесс (kill + wait — не оставляем зомби).
    pub fn shutdown(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// RSS процесса в МБ (порог PoC: ≤ 2.5 ГБ red). Unix: /proc/<pid>/statm;
    /// прочие платформы — нет данных (None).
    #[cfg(unix)]
    pub fn rss_mb(&self) -> Option<f64> {
        let pid = self.pid()?;
        let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
        let mut it = statm.split_whitespace();
        let _resident_pages = it.nth(1)?.parse::<u64>().ok()?;
        // второе поле statm = resident pages; страница = 4096 байт (обычно)
        let page = 4096.0;
        Some(_resident_pages as f64 * page / 1024.0 / 1024.0)
    }

    #[cfg(not(unix))]
    pub fn rss_mb(&self) -> Option<f64> {
        None
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_is_running_shutdown() {
        let cfg = SidecarConfig::new("/bin/sh").with_args(vec!["-c".into(), "sleep 300".into()]);
        let mut sc = Sidecar::spawn(&cfg).expect("spawn /bin/sh sleep");
        assert!(sc.is_running());
        assert!(sc.pid().is_some());
        sc.shutdown();
        assert!(!sc.is_running());
        assert_eq!(sc.pid(), None);
    }

    #[test]
    fn idle_shutdown_fires_only_after_limit() {
        let cfg = SidecarConfig::new("/bin/sh")
            .with_args(vec!["-c".into(), "sleep 300".into()])
            .with_idle_shutdown(Duration::from_millis(50));
        let mut sc = Sidecar::spawn(&cfg).expect("spawn");
        assert!(
            !sc.shutdown_if_idle(Duration::from_secs(3600)),
            "сразу — не должен"
        );
        std::thread::sleep(Duration::from_millis(80));
        assert!(
            sc.shutdown_if_idle(Duration::from_millis(50)),
            "после простоя — должен"
        );
        assert!(!sc.is_running());
    }

    #[test]
    fn touch_resets_idle_clock() {
        let mut sc = Sidecar::spawn(
            &SidecarConfig::new("/bin/sh").with_args(vec!["-c".into(), "sleep 300".into()]),
        )
        .expect("spawn");
        std::thread::sleep(Duration::from_millis(30));
        sc.touch();
        assert!(sc.idle_for() < Duration::from_millis(30));
        assert!(!sc.shutdown_if_idle(Duration::from_millis(30)));
        sc.shutdown();
    }

    #[test]
    fn drop_kills_child() {
        let cfg = SidecarConfig::new("/bin/sh").with_args(vec!["-c".into(), "sleep 300".into()]);
        let pid;
        {
            let sc = Sidecar::spawn(&cfg).expect("spawn");
            pid = sc.pid().unwrap();
        } // Drop
        std::thread::sleep(Duration::from_millis(50));
        // процесс завершён: /proc/<pid> исчезает не мгновенно, но statm уже
        // недоступен или процесс зомби; проверяем через kill(pid, 0) → ESRCH
        let alive = libc_kill_zero(pid);
        assert!(!alive, "процесс {pid} должен быть мёртв после Drop");
    }

    /// kill(pid, 0) без libc-зависимости: /proc/<pid>/stat читается только
    /// для живых (незомби) процессов.
    fn libc_kill_zero(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|s| !s.contains(") Z"))
            .unwrap_or(false)
    }
}
