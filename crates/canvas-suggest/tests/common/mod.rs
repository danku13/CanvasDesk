//! Общая инфраструктура golden-тестов: загрузка фикстур PoC.
//!
//! Фикстуры заморожены волной 1 (архив laya-poc-2026-09-27, harness
//! e6e666e) — происхождение и регенерация: `tests/fixtures/README.md`.

// Каждый интеграционный тест компилирует этот модуль отдельно и использует
// подмножество полей/методов — поля держим для полноты фикстуры.
#![allow(dead_code)]

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub split: String,
    pub task: String,
    pub context: String,
    #[serde(default)]
    pub options: Vec<FixOption>,
    pub golden: Golden,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct FixOption {
    pub id: String,
    #[serde(default)]
    pub desc: String,
}

#[derive(Debug, Deserialize)]
pub struct Golden {
    #[serde(default)]
    pub choice: Option<String>,
    #[serde(default)]
    pub show: Option<bool>,
}

/// Замороженная mm-проба (инференс laya-multilingual волны 1).
/// У show-задач probs нет; у части synth-прогонов нет task.
#[derive(Debug, Deserialize)]
pub struct MmRun {
    pub id: String,
    #[serde(default)]
    pub probs: Option<HashMap<String, f64>>,
}

impl Fixture {
    /// Флаг «переменных в скоупе физически < 8» (QC §5.6 волны 1).
    pub fn below_min_options(&self) -> bool {
        self.meta
            .as_ref()
            .and_then(|m| m.get("below_min_options"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// Основная метрика волны 1: choice-задачи, `n_options ≥ 8`,
/// без `below_min_options` (n: test 47 / dev 92 / pool 139).
pub fn main_filter(f: &Fixture) -> bool {
    f.golden.choice.is_some() && f.options.len() >= 8 && !f.below_min_options()
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn load_evalset() -> Vec<Fixture> {
    let mut out = Vec::new();
    for name in ["evalset/source_a.jsonl", "evalset/source_b.jsonl"] {
        let path = fixtures_dir().join(name);
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("не читается {path:?}: {e}"));
        for line in text.lines() {
            if !line.trim().is_empty() {
                out.push(
                    serde_json::from_str(line)
                        .unwrap_or_else(|e| panic!("разбор {name}: {e}\nстрока: {line}")),
                );
            }
        }
    }
    assert_eq!(out.len(), 226, "eval-сет волны 1: 175 + 51 фикстур");
    out
}

pub fn load_mm() -> HashMap<String, MmRun> {
    let path = fixtures_dir().join("laya_mm.json");
    let runs: Vec<MmRun> =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).expect("разбор laya_mm.json");
    assert_eq!(runs.len(), 226);
    runs.into_iter().map(|r| (r.id.clone(), r)).collect()
}

/// Счётчик p@1 по сплитам (test, dev, pool).
pub struct Hits {
    pub test: u32,
    pub dev: u32,
    pub pool: u32,
}

impl Hits {
    pub fn new() -> Self {
        Self {
            test: 0,
            dev: 0,
            pool: 0,
        }
    }

    pub fn add(&mut self, split: &str, hit: bool) {
        self.pool += u32::from(hit);
        match split {
            "test" => self.test += u32::from(hit),
            "dev" => self.dev += u32::from(hit),
            _ => {}
        }
    }
}
