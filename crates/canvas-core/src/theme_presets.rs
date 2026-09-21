//! Темы-пресеты как данные (FR-047, PRD-0006 этап D4 / F-8).
//!
//! Пресет — JSON-файл `design/tokens/themes/*.json` с ПОЛНЫМ набором
//! семантических слотов `ThemeColors` (36 ключей) + одна строка регистрации
//! в [`PRESETS`]. Рендер-код при добавлении пресета не меняется (G2):
//! `canvas_render::theme_presets::preset_theme` отображает разобранные
//! цвета в структуру по именам слотов универсально.
//!
//! Загрузка: `include_str!` (работа без ФС — wasm-гейт ADR-0011), разбор
//! и валидация serde_json (уже зависимость canvas-core), кэш [`OnceLock`]
//! — разбор происходит один раз на процесс, дальше выборка O(1)
//! (пути построения палитры вызываются каждый кадр).
//!
//! Инварианты FR-047:
//! - I-47.1: набор ключей `colors` строго равен [`REQUIRED_KEYS`] —
//!   лишний/недостающий/нечитаемый ключ = пресет не грузится (fallback —
//!   классическая тема), тест ловит;
//! - I-47.2: `label` в реестре == `label` в JSON (паритет-тест);
//! - I-47.3: контраст каждого пресета проверяется машиной G3 в
//!   canvas-render (тексты ≥ 4.5:1 к карточке, графика ≥ 3:1 к фону).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Зарегистрированный пресет: JSON встроен в бинарник на компиляции.
/// Регистрация нового пресета = JSON-файл + строка здесь (G2).
#[derive(Debug)]
pub struct ThemePreset {
    /// Идентификатор (`theme_preset` в config.toml), например `"nord"`.
    pub id: &'static str,
    /// Метка для UI (dropdown модалки настроек, FR-039).
    pub label: &'static str,
    /// Встроенный JSON `design/tokens/themes/<id>.json`.
    pub json: &'static str,
}

/// Реестр встроенных пресетов (состав — решение владельца, ответ на
/// открытый вопрос Q3 PRD-0006: 7 палитр, 5 тёмных + 2 светлых; Monokai,
/// GitHub, VSCode Dark Modern, Solarized Dark добавляются одной строкой
/// позже по тому же механизму).
pub const PRESETS: &[ThemePreset] = &[
    ThemePreset {
        id: "nord",
        label: "Nord",
        json: include_str!("../../../design/tokens/themes/nord.json"),
    },
    ThemePreset {
        id: "dracula",
        label: "Dracula",
        json: include_str!("../../../design/tokens/themes/dracula.json"),
    },
    ThemePreset {
        id: "catppuccin-mocha",
        label: "Mocha",
        json: include_str!("../../../design/tokens/themes/catppuccin-mocha.json"),
    },
    ThemePreset {
        id: "catppuccin-latte",
        label: "Latte",
        json: include_str!("../../../design/tokens/themes/catppuccin-latte.json"),
    },
    ThemePreset {
        id: "solarized-light",
        label: "Solarized",
        json: include_str!("../../../design/tokens/themes/solarized-light.json"),
    },
    ThemePreset {
        id: "tokyo-night",
        label: "Tokyo Night",
        json: include_str!("../../../design/tokens/themes/tokyo-night.json"),
    },
    ThemePreset {
        id: "gruvbox-dark",
        label: "Gruvbox",
        json: include_str!("../../../design/tokens/themes/gruvbox-dark.json"),
    },
];

/// Обязательные ключи `colors` — в точности слоты `ThemeColors`
/// (canvas-render). Порядок не значим; набор проверяется тестом на
/// соответствие структуре-эталону.
pub const REQUIRED_KEYS: &[&str] = &[
    // байтовые (Color / [u8;3])
    "background",
    "title",
    "icon",
    "body",
    "edge_label",
    "link",
    "quote",
    "code_text",
    "whatif_badge",
    "error",
    "hud",
    // [f32;3]
    "grid_minor",
    "grid_major",
    // [f32;4]
    "card_fill",
    "edge_edit_fill",
    "edge_label_fill",
    "menu_fill",
    "search_input_fill",
    "search_row_fill",
    "palette_row_fill",
    "palette_chip_fill",
    "palette_tile_fill",
    "palette_selected_fill",
    "palette_hover_fill",
    "palette_border",
    "gfm_code_fill",
    "gfm_quote_fill",
    "gfm_muted_fill",
    "group_fill",
    "group_border",
    "guide_align",
    "guide_grid",
    "accent",
    "selection_fill",
    "highlight",
    "whatif_fill",
    "stage_dim",
];

/// Сырой JSON-документ пресета (поля `$note`/`source`/`license` игнорируются).
#[derive(Debug, Deserialize)]
pub struct PresetDocument {
    /// Идентификатор (должен совпадать с id в реестре — тест).
    pub id: String,
    /// Метка UI.
    pub label: String,
    /// Слоты цвета: `#RRGGBB` или `#RRGGBBAA`.
    pub colors: HashMap<String, String>,
}

/// Разобранный пресет: цвета в sRGB 0..1 RGBA (единицы `ThemeColors`).
#[derive(Debug, Clone)]
pub struct ParsedPreset {
    /// Метка UI (из JSON; паритет с реестром — тест).
    pub label: String,
    /// Слоты → RGBA 0..1 (альфа #RRGGBB → 1.0).
    pub colors: HashMap<String, [f32; 4]>,
}

/// Поиск пресета по id (пустая/неизвестная строка — `None` → классическая
/// тема; ручная правка config.toml с устаревшим id деградирует мягко).
pub fn find(id: &str) -> Option<&'static ThemePreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Разбор hex-цвета: `#RRGGBB` (альфа 1.0) или `#RRGGBBAA`.
fn parse_hex(value: &str) -> Result<[f32; 4], String> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| format!("нет '#': {value:?}"))?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string());
    match hex.len() {
        6 => {
            let r = byte(0)?;
            let g = byte(2)?;
            let b = byte(4)?;
            Ok([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
        }
        8 => {
            let r = byte(0)?;
            let g = byte(2)?;
            let b = byte(4)?;
            let a = byte(6)?;
            Ok([
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ])
        }
        n => Err(format!("длина {n} != 6|8: {value:?}")),
    }
}

/// Разбор+валидация документа пресета (I-47.1): набор ключей строго
/// равен [`REQUIRED_KEYS`], каждый цвет — читаемый hex.
pub fn parse(preset: &ThemePreset) -> Result<ParsedPreset, String> {
    let doc: PresetDocument =
        serde_json::from_str(preset.json).map_err(|e| format!("{}: {e}", preset.id))?;
    if doc.id != preset.id {
        return Err(format!(
            "id в JSON {:?} != id в реестре {:?}",
            doc.id, preset.id
        ));
    }
    let mut colors = HashMap::with_capacity(REQUIRED_KEYS.len());
    for key in REQUIRED_KEYS {
        let raw = doc
            .colors
            .get(*key)
            .ok_or_else(|| format!("{}: нет ключа {key}", preset.id))?;
        let rgba = parse_hex(raw).map_err(|e| format!("{}/{}: {e}", preset.id, key))?;
        colors.insert((*key).to_string(), rgba);
    }
    for key in doc.colors.keys() {
        if !REQUIRED_KEYS.contains(&key.as_str()) {
            return Err(format!("{}: лишний ключ {key}", preset.id));
        }
    }
    Ok(ParsedPreset {
        label: doc.label,
        colors,
    })
}

/// Кэш разбора всех пресетов (один раз на процесс).
fn cache() -> &'static OnceLock<Vec<(&'static ThemePreset, Result<ParsedPreset, String>)>> {
    static CACHE: OnceLock<Vec<(&'static ThemePreset, Result<ParsedPreset, String>)>> =
        OnceLock::new();
    &CACHE
}

fn cached() -> &'static Vec<(&'static ThemePreset, Result<ParsedPreset, String>)> {
    cache().get_or_init(|| PRESETS.iter().map(|p| (p, parse(p))).collect())
}

/// Разобранный пресет по id: `Some` — успех, `None` — неизвестный id или
/// невалидный документ (деградация к классической теме на вызывающей
/// стороне; невалидность пресета ловится тестами).
pub fn parsed(id: &str) -> Option<&'static ParsedPreset> {
    cached()
        .iter()
        .find(|(p, _)| p.id == id)
        .and_then(|(_, r)| r.as_ref().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все зарегистрированные пресеты разбираются без ошибок.
    #[test]
    fn all_registered_presets_parse() {
        for (preset, result) in cached() {
            let parsed = result.as_ref().unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(parsed.label, preset.label, "паритет label (I-47.2)");
        }
    }

    /// Реестр: id уникальны, JSON каждого встроен непустым.
    #[test]
    fn registry_ids_unique_and_nonempty() {
        for (i, a) in PRESETS.iter().enumerate() {
            assert!(!a.json.is_empty());
            assert!(!a.label.is_empty());
            for b in &PRESETS[i + 1..] {
                assert_ne!(a.id, b.id, "дубликат id {a:?}");
            }
        }
        assert_eq!(PRESETS.len(), 7, "состав Q3 (решение владельца FR-047)");
    }

    /// I-47.1: недостающий/лишний/нечитаемый ключ — ошибка разбора.
    #[test]
    fn validation_rejects_bad_documents() {
        let fill = |colors: String| {
            r#"{"id":"t","label":"T","colors":{COLORS}}"#.replace("COLORS", &colors)
        };
        let make = |colors: String| ThemePreset {
            id: "t",
            label: "T",
            json: Box::leak(fill(colors).into_boxed_str()),
        };
        let full: String = REQUIRED_KEYS
            .iter()
            .map(|k| format!(r##""{k}":"#112233","##))
            .collect();
        let full = full.trim_end_matches(',').to_string();

        assert!(
            parse(&make(full.clone())).is_ok(),
            "полный набор разбирается"
        );

        // Недостающий ключ
        let without_first: String = REQUIRED_KEYS[1..]
            .iter()
            .map(|k| format!(r##""{k}":"#112233","##))
            .collect();
        let without_first = without_first.trim_end_matches(',').to_string();
        assert!(
            parse(&make(without_first)).is_err(),
            "нет ключа {} — ошибка",
            REQUIRED_KEYS[0]
        );

        // Лишний ключ
        assert!(parse(&make(format!("{full},\"alien\":\"#000000\""))).is_err());

        // Нечитаемый hex
        let first_key = REQUIRED_KEYS[0];
        let broken = full.replace(
            &format!(r##""{first_key}":"#112233"##),
            &format!(r##""{first_key}":"zzz"##),
        );
        assert!(parse(&make(broken)).is_err(), "нечитаемый hex — ошибка");
    }

    /// hex: 6 и 8 цифр, альфа по умолчанию 1.0.
    #[test]
    fn hex_parsing_forms() {
        assert_eq!(
            parse_hex("#112233").unwrap(),
            [
                0x11 as f32 / 255.0,
                0x22 as f32 / 255.0,
                0x33 as f32 / 255.0,
                1.0
            ]
        );
        assert_eq!(parse_hex("#11223344").unwrap()[3], 0x44 as f32 / 255.0);
        assert!(parse_hex("112233").is_err());
        assert!(parse_hex("#1122").is_err());
    }

    /// find: пустая и неизвестная строки — None, известные — Some.
    #[test]
    fn find_lookup() {
        assert!(find("").is_none());
        assert!(find("unknown").is_none());
        assert_eq!(find("nord").unwrap().id, "nord");
        for preset in PRESETS {
            assert!(parsed(preset.id).is_some());
        }
    }
}
