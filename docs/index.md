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

Каталог PRD — [`prd/`](prd/README.html): правила ведения, единый
[шаблон](prd/TEMPLATE.html) (CJM — обязательный раздел) и индекс документов.

| Документ | Содержание |
|---|---|
| [PRD-0001: DuckDB-коннектор данных](prd/prd-0001-duckdb-data-connector.html) | PoC подключения к файлам/БД (CSV/Parquet/JSON, SQLite/PG/MySQL), инспектор колонок, агрегаты min/max/avg/p50/p90/p95 → вводные мат.модели |
| [PRD-0002: LOD-визуализация связей и main stage](prd/prd-0002-edge-lod-main-stage.html) | Пучки связей одной линией с толщиной по весу входов, режим main stage (≤ 70% вьюпорта), выход Esc/кликом по фону |
| [PRD-0003: Всеядный импорт описаний архитектуры](prd/prd-0003-universal-architecture-import.html) | Импорт машиночитаемых описаний (draw.io/UML/PlantUML/Mermaid/DOT, terraform/ansible/compose) → ноды, связи и группы канваса |
| [PRD-0004: Единая анатомия ноды](prd/prd-0004-node-anatomy-restructure.html) | Реструктуризация объекта node по практикам node-style UI: зоны A–E (хедер, порт-рейлы, тело, полоса результата, статусный слой), LOD L0/L1/L2, компакт-режим |

PRD — документы требований продукта (проблема → цели → пользователи и CJM →
user stories → решение → non-goals → roadmap). CJM с трассировкой точек отвала —
обязательный раздел каждого PRD. При переходе к реализации каждый PRD оформляется
отдельным FR в `change-requests/` по шаблону `cr-template.md`.

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
