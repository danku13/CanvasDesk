# CanvasDesk — документация

**CanvasDesk** — визуальная система математического моделирования (ADR-0007):
бесконечный зумируемый канвас, на котором исполняемые модели собираются из
расчётных нод (Numi-листы, встроенные шаблоны), значения проливаются по
value-связям (DAG-движок), а ИИ-агент строит и проверяет модели через MCP.

- **Веб-версия (wasm, M8):** [приложение `/app`](/CanvasDesk/app/) —
  Chromium/Edge (WebGPU), описание сборки и запуска — в
  [README репозитория](https://github.com/danku13/CanvasDesk#readme).
- **Пользовательская документация** (установка, горячие клавиши, шаблоны,
  расчёты, FAQ): [`user-docs/` в репозитории][user-docs].

[user-docs]: https://github.com/danku13/CanvasDesk/tree/main/user-docs

## Основные документы

| Документ | Содержание |
|---|---|
| [SPEC.md](SPEC.html) | Спецификация проекта: стек, модель данных, рендер, виджеты |
| [WIDGETS.md](WIDGETS.html) | Виджеты: рантайм, SDK, permissions, форматы пакетов |
| [ACCEPTANCE.md](ACCEPTANCE.html) | Приёмки задач и milestone'ов (T#, M#, волны FR/CR) |
| [RECIPES.md](RECIPES.html) | Рецепты встраивания в десктоп (R1–R17) |
| [DEMO.md](DEMO.html) | Демо-стенд под Linux: скриншоты и GIF |
| [BYOK.md](BYOK.html) | Подключение своей LLM-модели (BYOK) |
| [DEPENDENCIES.md](DEPENDENCIES.html) | Заметки о зависимостях |
| [ENVIRONMENT.md](ENVIRONMENT.html) | Окружение разработки и сборки |

## PRD — требования продукта

| Документ | Содержание |
|---|---|
| [PRD-0001: DuckDB-коннектор данных](PRD-duckdb-connector.html) | PoC подключения к файлам/БД (CSV/Parquet/JSON, SQLite/PG/MySQL), инспектор таблиц и колонок, агрегаты min/max/avg/p50/p90/p95 и перенос их в мат.модель |
| [PRD-0002: LOD-визуализация связей и main stage](PRD-edge-lod.html) | Пучки связей как одна линия с толщиной по весу входов, режим фокусировки main stage (≤ 70% вьюпорта), выход по Esc/клику |

PRD — документы требований продукта (проблема → цели → user stories → решение → non-goals → roadmap). При переходе к реализации каждый PRD оформляется отдельным FR в `change-requests/` по шаблону `cr-template.md`.

## Разделы

- [`adr/`](adr/) — архитектурные решения (ADR-0001…0012, индекс — `adr/README.md`).
- [Индекс CR/FR](change-requests/index-cr-fr.html) — постановки задач и
  дефектов, статусы, история изменений.
- [`plans/wasm-port.html`](plans/wasm-port.html) — план wasm-порта (M8);
  рядом — кроссплатформенность (M7) и продуктовый роадмап.
- [`architecture/math-computing-stack.html`](architecture/math-computing-stack.html) —
  архитектурный обзор стека математических вычислений.
- [`articles/fr-013-silent-calculator.html`](articles/fr-013-silent-calculator.html) —
  статьи («тихий калькулятор» Numi и др.).
- [`interface-objects/node.html`](interface-objects/node.html) —
  спецификации объектов интерфейса (нода, связь, поиск, миникарта…).
- [`devlog/M5-widgets.html`](devlog/M5-widgets.html) — девлоги milestone'ов.

Исходники документации — в репозитории
[danku13/CanvasDesk](https://github.com/danku13/CanvasDesk), каталог `docs/`.
Сайт собирается GitHub Pages (Jekyll) при каждом изменении `docs/`.
