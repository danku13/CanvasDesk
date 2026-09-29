//! Манифест весов (sha256-пины) и smoke-проверка sidecar при старте.
//!
//! Пины защищают от дрейфа версии/весов (риск-лист FR-079 §9: проекту
//! Laya 10 дней): смена весов меняет ответы mm → fusion деградирует
//! молча. Smoke ловит это явно: известная фикстура должна давать
//! известный top-1 (блокер по плану волны 1 §10).

use crate::laya::client::LayaClient;
use crate::types::OptionDesc;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// sha256 файла (hex, lowercase).
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Сверить файлы каталога с пинами `(имя файла, ожидаемый sha256)`.
/// Ошибка содержит имя несовпавшего файла и обе суммы.
pub fn verify_pins(dir: &Path, pins: &[(String, String)]) -> Result<(), String> {
    for (name, expected) in pins {
        let path = dir.join(name);
        let actual = sha256_file(&path).map_err(|e| format!("не читается {name}: {e}"))?;
        if &actual != expected {
            return Err(format!(
                "пин не совпал для {name}: ожидался {expected}, получен {actual}"
            ));
        }
    }
    Ok(())
}

/// Smoke-фикстура: документ + опции + ожидаемый top-1.
/// Продуктовый источник — замороженная проба волны 1
/// (`A-unit-economics-margin-ru-C3`, 19 опций, ожидание ue-gross-margin);
/// живёт рядом с весами в `laya-manifest.json` (S3 подключение).
#[derive(Debug, Clone)]
pub struct SmokeFixture {
    pub document: String,
    pub options: Vec<OptionDesc>,
    pub expect_top1: String,
}

/// Прогнать smoke через клиент: top-1 ответа mm должен совпасть с эталоном.
/// Несовпадение = дрейф версии/весов → продукт деградирует на lex + warn
/// (S2-гейт: smoke обязателен до первого боевого запроса).
pub fn run_smoke(client: &LayaClient, fx: &SmokeFixture) -> Result<(), String> {
    let answer = client
        .choice(&fx.document, &fx.options)
        .map_err(|e| format!("smoke-запрос не прошёл: {e}"))?;
    let top1 = answer
        .probs
        .first()
        .map(|(id, _)| id.as_str())
        .unwrap_or("");
    if top1 == fx.expect_top1 {
        Ok(())
    } else {
        Err(format!(
            "smoke-дрейф: top-1 «{top1}» ≠ ожидание «{}» (версия/весы sidecar изменились)",
            fx.expect_top1
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        // NIST-вектор: sha256("abc") = ba7816bf…
        let dir = std::env::temp_dir().join("canvas-suggest-sha-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("abc.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn verify_pins_detects_mismatch() {
        let dir = std::env::temp_dir().join("canvas-suggest-pins-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("weights.bin"), b"payload-v1").unwrap();
        let good = sha256_file(&dir.join("weights.bin")).unwrap();
        assert_eq!(
            verify_pins(&dir, &[("weights.bin".into(), good.clone())]),
            Ok(())
        );
        assert!(verify_pins(&dir, &[("weights.bin".into(), "deadbeef".into())]).is_err());
        assert!(verify_pins(&dir, &[("missing.bin".into(), good)]).is_err());
        let _ = std::fs::remove_file(dir.join("weights.bin"));
    }
}
