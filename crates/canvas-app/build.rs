//! Встраивание версии сборки в бинарник: git-коммит и флаг «рабочая копия
//! грязная» уходят в env-константы, main() печатает их в лог первой строкой —
//! по логу видно, какую именно сборку запустили (диагностика после
//! пересборки). Git недоступен/не репозиторий — «unknown», сборка не ломается.

use std::process::Command;

/// Вывод команды git: stdout как строка при успехе, иначе None.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    // Короткий хэш HEAD; вне репозитория/без git — «unknown»
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    // «Грязная» = есть изменения в ОТСЛЕЖИВАЕМЫХ файлах (untracked не считаем:
    // marketing/ и .zcode/ в репозитории не закоммичены, а флаг нужен, чтобы
    // отличать «сборка ровно с коммита» от «с локальными правками»)
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|out| !out.is_empty());
    println!("cargo:rustc-env=CANVASDESK_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=CANVASDESK_GIT_DIRTY={dirty}");
    // Пересборка при смене коммита/индекса — иначе env застынет на первой
    // сборке (cargo по умолчанию не следит за .git)
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
