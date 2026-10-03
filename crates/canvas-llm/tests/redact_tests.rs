//! FR-LLM-A / Q1 — интеграционные тесты `redact_context()`.
//!
//! Проверяют пример из PRD-0010 F-2.9 (явный маркер `<redacted>`,
//! сохраняем unit) и edge-cases (пустая строка, без значения, с пробелами,
//! Multimodal-варианты). Тесты не зависят от feature `l1-llm` — `redact`
//! модуль pure-std.

use canvas_llm::compliance::{DataResidency, PrivacyMode};
use canvas_llm::redact_context;

#[test]
fn off_mode_passthrough_local() {
    let ctx = "price=10руб, cogs=4руб";
    let out = redact_context(ctx, PrivacyMode::Off);
    assert_eq!(out, ctx, "Off mode = passthrough");
}

#[test]
fn off_mode_passthrough_selfhosted() {
    let ctx = "price=10руб";
    let mode = DataResidency::SelfHosted.privacy_mode();
    assert_eq!(mode, PrivacyMode::Off);
    assert_eq!(redact_context(ctx, mode), ctx);
}

#[test]
fn redact_prd_example_exact() {
    // Пример из PRD-0010 F-2.9 / Stream-A task description.
    let ctx = "price=10руб, cogs=4руб, months=36, cac=120руб";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(
        out,
        "price=<redacted> руб, cogs=<redacted> руб, months=<redacted>, cac=<redacted> руб"
    );
}

#[test]
fn redact_cloud_mode_from_data_residency() {
    let mode = DataResidency::Cloud.privacy_mode();
    assert_eq!(mode, PrivacyMode::Redact);
    let out = redact_context("price=10руб", mode);
    assert_eq!(out, "price=<redacted> руб");
}

#[test]
fn redact_decimal_numbers() {
    let out = redact_context("price=10.5руб, ratio=0.85", PrivacyMode::Redact);
    assert_eq!(out, "price=<redacted> руб, ratio=<redacted>");
}

#[test]
fn redact_negative_numbers() {
    let out = redact_context("delta=-2.0%", PrivacyMode::Redact);
    assert_eq!(out, "delta=<redacted> %");
}

#[test]
fn redact_various_units() {
    let ctx = "cost=120$, profit=15%, hours=8ч, days=30дн, weight=5кг, dist=10м";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(
        out,
        "cost=<redacted> $, profit=<redacted> %, hours=<redacted> ч, days=<redacted> дн, weight=<redacted> кг, dist=<redacted> м"
    );
}

#[test]
fn redact_data_units_case_insensitive() {
    let ctx = "size=100MB, ram=8GB, disk=1TB, cache=512KB";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(
        out,
        "size=<redacted> MB, ram=<redacted> GB, disk=<redacted> TB, cache=<redacted> KB"
    );
}

#[test]
fn redact_no_unit_only_number() {
    let ctx = "months=36, year=2026";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "months=<redacted>, year=<redacted>");
}

#[test]
fn redact_preserves_unrelated_text() {
    let ctx = "Hello world, just a free text without key=value patterns.";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, ctx);
}

#[test]
fn redact_no_value_after_eq_preserved() {
    let ctx = "price=, cogs=4руб";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "price=, cogs=<redacted> руб");
}

#[test]
fn redact_underscore_identifier() {
    let ctx = "unit_cac=120руб, monthly_revenue=50000руб";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(
        out,
        "unit_cac=<redacted> руб, monthly_revenue=<redacted> руб"
    );
}

#[test]
fn redact_long_units_match_first() {
    // `мес` должно матчиться как единое целое, а не `м` + `ес`.
    // `мес` (3 буквы) vs `м` (1 буква) — сортировка UNITS по убыванию длины.
    let ctx = "duration=36мес, length=10м";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "duration=<redacted> мес, length=<redacted> м");
}

#[test]
fn redact_with_spaces_around_eq() {
    // Resilience: `price = 10руб` — пробелы вокруг `=`.
    let ctx = "price = 10руб";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "price = <redacted> руб");
}

#[test]
fn redact_empty_string() {
    assert_eq!(redact_context("", PrivacyMode::Redact), "");
    assert_eq!(redact_context("", PrivacyMode::Off), "");
}

#[test]
fn redact_multiple_matches_mixed() {
    let ctx = "[canvas] 3 nodes: revenue=50000руб, costs=35000руб, months=12";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(
        out,
        "[canvas] 3 nodes: revenue=<redacted> руб, costs=<redacted> руб, months=<redacted>"
    );
}

#[test]
fn redact_only_replaces_key_value_pattern() {
    // Свободные числа в тексте (без `key=`) — НЕ заменяются.
    // Только шаблон `key=value[unit]` считается приватным.
    let ctx = "In 2026 the price was 100руб and 50 in stock.";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, ctx, "free numbers without key= pattern preserved");
}

#[test]
fn redact_usd_unit() {
    let ctx = "budget=1000usd, price=500usd";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "budget=<redacted> usd, price=<redacted> usd");
}

#[test]
fn redact_ruble_symbol() {
    let ctx = "price=100₽";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "price=<redacted> ₽");
}

#[test]
fn redact_dollar_symbol() {
    let ctx = "price=100$";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "price=<redacted> $");
}

#[test]
fn redact_percent_unit() {
    let ctx = "margin=85%";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(out, "margin=<redacted> %");
}

#[test]
fn redact_time_units() {
    let ctx = "elapsed=120сек, timeout=30мин, duration=2ч";
    let out = redact_context(ctx, PrivacyMode::Redact);
    assert_eq!(
        out,
        "elapsed=<redacted> сек, timeout=<redacted> мин, duration=<redacted> ч"
    );
}
