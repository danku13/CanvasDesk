//! canvas-app — приложение: event loop, команды, UI-состояние, main().

fn main() {
    println!("canvasdesk {}", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    /// Версия пакета — валидный semver вида x.y.z.
    #[test]
    fn version_is_semver() {
        let version = env!("CARGO_PKG_VERSION");
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3, "версия должна быть вида x.y.z: {version}");
        for part in parts {
            assert!(
                part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty(),
                "компонент версии не числовой: {part}"
            );
        }
    }
}
