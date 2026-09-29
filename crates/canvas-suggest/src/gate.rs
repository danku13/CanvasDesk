//! Show-гейт: показывать ли подсказку вообще (FR-079 §2.4).
//!
//! Детерминированное правило (порт `baseline_lex.py: noul`, волна 1;
//! точность 37/39 = 0.949 на show-фикстурах): показывать, если контекст
//! длиннее `min_ctx_len` символов (кодпоинтов) И есть секция `[up]`
//! или `[down]` (у дырки есть соседи).
//!
//! Почему правило, а не нейро-гейт: уверенность mm zero-shot не несёт
//! информации о правильности (волна 3 §7: Platt a≈0, потолок precision
//! confidence/margin-гейта ниже порога) — пересмотр только после H8.

/// Порог длины контекста по умолчанию (символов; конфиг `show_gate_min_ctx`).
pub const DEFAULT_MIN_CTX_LEN: usize = 120;

/// Показывать ли подсказку для данного контекста (формат А).
pub fn should_show(document: &str, min_ctx_len: usize) -> bool {
    document.chars().count() > min_ctx_len
        && (document.contains("[up]") || document.contains("[down]"))
}

/// С дефолтным порогом.
pub fn should_show_default(document: &str) -> bool {
    should_show(document, DEFAULT_MIN_CTX_LEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG_WITH_UP: &str = "[canvas] 12 nodes; cats: unit-economics(6) product-analytics(3) misc(3); vars: price=10руб, cogs=4руб, months=36, cac=120руб\n[node] title=\"Маржа\"; editing=\"валовая\"\n[up] \"Продуктовая единица\" out: cogs, price\n[down] \"LTV\" needs: margin\n[down] \"Окупаемость\" needs: margin, months";

    #[test]
    fn shows_when_long_and_linked() {
        assert!(should_show_default(LONG_WITH_UP));
        assert!(should_show(LONG_WITH_UP, 50));
    }

    #[test]
    fn hides_when_isolated() {
        // длинный, но без соседей
        let isolated = "[canvas] 8 nodes; cats: misc(8); vars: x=1\n[node] title=\"Заметка\"; editing=\"текст достаточно длинный чтобы пройти порог\"";
        assert!(!should_show_default(isolated));
    }

    #[test]
    fn hides_when_short() {
        // с соседями, но короткий
        let short = "[canvas] 2 nodes\n[node] title=\"\"\n[up] \"A\" out: x";
        assert!(!should_show_default(short));
    }
}
