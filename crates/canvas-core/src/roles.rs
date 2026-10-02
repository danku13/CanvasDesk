//! FR-087: реестр рабочих ролей пользователя (class) — персональная
//! фильтрация подсказок шаблонных нод и шаблонных схем.
//!
//! Роль — идентификатор из [`ROLES`] (например `architect`), сохраняется в
//! `config.toml` (`Settings::role`, serde default `default` = роль не
//! выбрана → все подсказки). Выбор на первом экране web-версии (пикер
//! языка → шаг роли, `crates/canvas-web/index.html`) и в табе «Профиль»
//! модалки настроек (FR-039).
//!
//! Инварианты:
//! 1. Реестр — чистые данные (`ROLES`), ≤ 10 ролей + `default`; расширение
//!    — одна строка в массиве (id/имена/описание/категории), без правок
//!    логики.
//! 2. Неизвестный id (ручная правка конфига, переименование роли) —
//!    деградирует мягко: поведение как у `default` (все подсказки).
//! 3. Категория `custom` (свои шаблоны FR-020) и категория схем
//!    `onboarding` (вводные туториалы) видны ВСЕГДА — выбор роли не
//!    прячет личный контент пользователя и обучающие схемы.
//! 4. Явный ручной фильтр категорий в настройках хранится в
//!    `Settings::template_categories`/`Settings::scheme_categories`
//!    (материализация поверх роли); здесь — только дефолт по роли.

/// Идентификатор роли по умолчанию — «роль не выбрана» (все подсказки,
/// максимальный шум; фильтр сознательно выключен).
pub const DEFAULT_ROLE: &str = "default";

/// Категория шаблонов, видимая при любой роли: свои шаблоны FR-020.
pub const ALWAYS_VISIBLE_TEMPLATE_CATEGORY: &str = "custom";
/// Категория схем, видимая при любой роли: вводные/обучающие схемы.
pub const ALWAYS_VISIBLE_SCHEME_CATEGORY: &str = "onboarding";

/// Спецификация роли: имена/описание для UI (двуязычие — паттерн
/// `TemplateManifest::name_ru`/`name_en`, FR-019) + видимые категории
/// подсказок по умолчанию (сырые токены категорий реестров).
#[derive(Debug, Clone, PartialEq)]
pub struct RoleSpec {
    /// Идентификатор (snake_case, значение `Settings::role`).
    pub id: &'static str,
    /// Русское имя («Архитектор») — `name_ru`-паттерн FR-019.
    pub name_ru: &'static str,
    /// Английское имя («Architect»).
    pub name_en: &'static str,
    /// Краткое описание для пикера/настроек: какими подсказками обогатится
    /// интерфейс (ru).
    pub description_ru: &'static str,
    /// То же (en).
    pub description_en: &'static str,
    /// Видимые категории шаблонных нод (дефолт роли; пусто = все).
    pub template_categories: &'static [&'static str],
    /// Видимые категории шаблонных схем (дефолт роли; пусто = все).
    pub scheme_categories: &'static [&'static str],
}

impl RoleSpec {
    /// Имя по языку интерфейса (паттерн `TemplateManifest::display_name`).
    pub fn display_name(&self, language: crate::Language) -> &'static str {
        match language {
            crate::Language::Ru => self.name_ru,
            crate::Language::En => self.name_en,
        }
    }

    /// Описание по языку интерфейса.
    pub fn display_description(&self, language: crate::Language) -> &'static str {
        match language {
            crate::Language::Ru => self.description_ru,
            crate::Language::En => self.description_en,
        }
    }
}

/// Реестр ролей (≤ 10, вместе с `default`). Порядок = порядок показа в
/// пикере первого запуска и dropdown'е настроек. Расширение — добавить
/// элемент; UI читает реестр, отдельных правок не нужно.
pub const ROLES: &[RoleSpec] = &[
    RoleSpec {
        id: DEFAULT_ROLE,
        name_ru: "Не выбрана",
        name_en: "Not selected",
        description_ru: "Все подсказки шаблонов и схем без фильтра.",
        description_en: "All template and scheme hints, no filtering.",
        // default = «всё»: пустые списки трактуются фильтром как «все».
        template_categories: &[],
        scheme_categories: &[],
    },
    RoleSpec {
        id: "architect",
        name_ru: "Архитектор",
        name_en: "Architect",
        description_ru: "Подсказки шаблонов: бэкенд и сеть. Схемы: архитектура и планирование мощностей.",
        description_en: "Template hints: backend and network. Schemes: architecture and capacity planning.",
        template_categories: &["backend", "network"],
        scheme_categories: &["architecture", "planning"],
    },
    RoleSpec {
        id: "developer",
        name_ru: "Разработчик",
        name_en: "Developer",
        description_ru: "Подсказки шаблонов: бэкенд и сеть. Схемы: архитектура сервисов.",
        description_en: "Template hints: backend and network. Schemes: service architecture.",
        template_categories: &["backend", "network"],
        scheme_categories: &["architecture"],
    },
    RoleSpec {
        id: "product-manager",
        name_ru: "Продакт-менеджер",
        name_en: "Product Manager",
        description_ru: "Подсказки шаблонов: продуктовая аналитика и юнит-экономика. Схемы: бизнес-модели и фреймворки.",
        description_en: "Template hints: product analytics and unit economics. Schemes: business models and frameworks.",
        template_categories: &["product-analytics", "unit-economics"],
        scheme_categories: &["business", "framework"],
    },
    RoleSpec {
        id: "analyst",
        name_ru: "Аналитик",
        name_en: "Analyst",
        description_ru: "Подсказки шаблонов: продуктовая аналитика и юнит-экономика. Схемы: бизнес, фреймворки и планирование.",
        description_en: "Template hints: product analytics and unit economics. Schemes: business, frameworks and planning.",
        template_categories: &["product-analytics", "unit-economics"],
        scheme_categories: &["business", "framework", "planning"],
    },
    RoleSpec {
        id: "cio",
        name_ru: "CIO",
        name_en: "CIO",
        description_ru: "Подсказки шаблонов: бэкенд-инфраструктура и юнит-экономика. Схемы: бизнес, архитектура и планирование.",
        description_en: "Template hints: backend infrastructure and unit economics. Schemes: business, architecture and planning.",
        template_categories: &["backend", "unit-economics"],
        scheme_categories: &["business", "architecture", "planning"],
    },
    RoleSpec {
        id: "cto",
        name_ru: "CTO",
        name_en: "CTO",
        description_ru: "Подсказки шаблонов: бэкенд, сеть и юнит-экономика. Схемы: архитектура, бизнес и планирование.",
        description_en: "Template hints: backend, network and unit economics. Schemes: architecture, business and planning.",
        template_categories: &["backend", "network", "unit-economics"],
        scheme_categories: &["architecture", "business", "planning"],
    },
    RoleSpec {
        id: "founder",
        name_ru: "Основатель / CEO",
        name_en: "Founder / CEO",
        description_ru: "Подсказки шаблонов: юнит-экономика. Схемы: бизнес-модели, планирование и фреймворки.",
        description_en: "Template hints: unit economics. Schemes: business models, planning and frameworks.",
        template_categories: &["unit-economics"],
        scheme_categories: &["business", "planning", "framework"],
    },
];

/// Реестр не длиннее 10 позиций (требование владельца: «ролей не более 10
/// штук»; инвариант проверяется тестом).
pub const MAX_ROLES: usize = 10;

/// Поиск роли по id (`default`/неизвестное — None: поведение «все
/// подсказки» обрабатывает вызывающий, см. [`template_category_visible`]).
pub fn find(id: &str) -> Option<&'static RoleSpec> {
    ROLES.iter().find(|role| role.id == id)
}

/// Имя роли по id и языку; неизвестный id — «Не выбрана» (мягкая
/// деградация инварианта 2).
pub fn display_name(id: &str, language: crate::Language) -> &'static str {
    find(id)
        .map(|role| role.display_name(language))
        .unwrap_or_else(|| {
            find(DEFAULT_ROLE)
                .expect("default в реестре")
                .display_name(language)
        })
}

/// Видимость категории шаблонных нод для роли: `default`/неизвестная роль
/// — всё; иначе — категория в списке роли ИЛИ всегда-видимая
/// ([`ALWAYS_VISIBLE_TEMPLATE_CATEGORY`]).
pub fn template_category_visible(role: &str, category: &str) -> bool {
    if category == ALWAYS_VISIBLE_TEMPLATE_CATEGORY {
        return true;
    }
    match find(role) {
        // Пустой список трактуется как «все» (default).
        Some(spec) if !spec.template_categories.is_empty() => {
            spec.template_categories.contains(&category)
        }
        _ => true,
    }
}

/// Видимость категории шаблонных схем для роли (симметрично
/// [`template_category_visible`]).
pub fn scheme_category_visible(role: &str, category: &str) -> bool {
    if category == ALWAYS_VISIBLE_SCHEME_CATEGORY {
        return true;
    }
    match find(role) {
        Some(spec) if !spec.scheme_categories.is_empty() => {
            spec.scheme_categories.contains(&category)
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_within_limit_and_unique() {
        assert!(ROLES.len() <= MAX_ROLES, "ролей не больше {MAX_ROLES}");
        assert!(ROLES.len() >= 8, "ожидаем не меньше 8 ролей с default");
        let mut ids: Vec<&str> = ROLES.iter().map(|r| r.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "id ролей уникальны");
        assert!(find(DEFAULT_ROLE).is_some(), "default в реестре");
        // Требуемый владельцем набор ролей присутствует.
        for expected in [
            "architect",
            "developer",
            "product-manager",
            "analyst",
            "cio",
            "cto",
        ] {
            assert!(find(expected).is_some(), "роли {expected} нет в реестре");
        }
    }

    #[test]
    fn default_and_unknown_role_see_everything() {
        for category in [
            "backend",
            "network",
            "unit-economics",
            "product-analytics",
            "custom",
        ] {
            assert!(template_category_visible(DEFAULT_ROLE, category));
            assert!(template_category_visible("no-such-role", category));
        }
        for category in [
            "architecture",
            "business",
            "framework",
            "onboarding",
            "planning",
        ] {
            assert!(scheme_category_visible(DEFAULT_ROLE, category));
            assert!(scheme_category_visible("no-such-role", category));
        }
    }

    #[test]
    fn role_filters_categories() {
        // Архитектор: бэкенд/сеть да, юнит-экономика и аналитика — нет.
        assert!(template_category_visible("architect", "backend"));
        assert!(template_category_visible("architect", "network"));
        assert!(!template_category_visible("architect", "unit-economics"));
        assert!(!template_category_visible("architect", "product-analytics"));
        assert!(scheme_category_visible("architect", "architecture"));
        assert!(!scheme_category_visible("architect", "business"));
        // Продакт: аналитика/юнит-экономика да, бэкенд — нет.
        assert!(template_category_visible(
            "product-manager",
            "product-analytics"
        ));
        assert!(template_category_visible(
            "product-manager",
            "unit-economics"
        ));
        assert!(!template_category_visible("product-manager", "backend"));
    }

    #[test]
    fn always_visible_categories_survive_any_role() {
        for role in ROLES {
            assert!(
                template_category_visible(role.id, ALWAYS_VISIBLE_TEMPLATE_CATEGORY),
                "custom-шаблоны скрыты для {}",
                role.id
            );
            assert!(
                scheme_category_visible(role.id, ALWAYS_VISIBLE_SCHEME_CATEGORY),
                "onboarding-схемы скрыты для {}",
                role.id
            );
        }
    }

    #[test]
    fn display_name_fallback_for_unknown_role() {
        assert_eq!(display_name("architect", crate::Language::Ru), "Архитектор");
        assert_eq!(display_name("architect", crate::Language::En), "Architect");
        // Неизвестная роль — подпись default, не паника.
        assert_eq!(display_name("???", crate::Language::Ru), "Не выбрана");
    }

    #[test]
    fn role_descriptions_are_meaningful() {
        for role in ROLES {
            assert!(
                role.description_ru.len() > 20,
                "пустое описание ru: {}",
                role.id
            );
            assert!(
                role.description_en.len() > 20,
                "пустое описание en: {}",
                role.id
            );
        }
    }
}
