# AI-функции CanvasDesk

> CanvasDesk поддерживает три AI-функции: автодополнение шаблонов (Suggest),
> генерация графов из текста (Graph Builder), и агентные операции через MCP
> (Agent Panel). Каждая функция работает с любым LLM-провайдером на ваш выбор.

## Обзор

| Функция | Что делает | Когда вызывается |
|---|---|---|
| **Suggest** | Подсказывает подходящий шаблон ноды или генерирует 3 варианта кастомных нод | При редактировании ноды (автоматически, с задержкой 500мс) |
| **Graph Builder** | Превращает текст в граф нод + связей | По запросу из меню или `.byok`-файла |
| **Agent Panel** | Выполняет операции на канвасе через MCP-инструменты по текстовому запросу | При открытии панели (Ctrl+I) и отправке запроса |

## Провайдеры

CanvasDesk поддерживает 8 режимов работы AI. Выберите подходящий в
**Настройки → AI and Models** (9-й таб):

| Режим | Качество | Стоимость | Privacy | Offline |
|---|---|---|---|---|
| **ChatGPT OAuth** | Высокое | Бесплатно (подписка ChatGPT) | Данные уходят OpenAI | Нет |
| **BYOK (свой ключ)** | Высшее | Pay-per-token | Данные уходят провайдеру | Нет |
| **Ollama (local)** | Среднее | Бесплатно | Данные не уходят | Да |
| **Laya sidecar** | Низкое | Бесплатно | Данные не уходят | Да |
| **Self-hosted** | Высокое | Зависит | Данные в контуре | Нет |
| **Off** | — | — | — | — |

### Per-feature настройка

Каждую функцию можно настроить отдельно:
- **Suggest**: BYOK / Ollama / Laya / Off (ChatGPT OAuth недоступен — rate limits)
- **Graph Builder**: ChatGPT OAuth / BYOK / Ollama / Off
- **Agent Panel**: ChatGPT OAuth / BYOK / Ollama / Off

## Privacy и данные

### Что отправляется

При работе Suggest отправляется **только контекст редактируемой ноды**:
- Заголовок ноды
- Переменные канваса (значения маскируются: `price=<redacted> руб`)
- Соседи (up/down) с их именами и потребностями

**Не отправляется:** содержимое других нод, файлы, ключи, весь канвас.

### Privacy-режимы (Data residency)

| Режим | Описание |
|---|---|
| **Local only** | Данные не покидают машину. Работает offline (Laya/Ollama). Качество ниже cloud. |
| **Cloud** | Контекст уходит провайдеру (ChatGPT/BYOK). Значения маскируются: `price=<redacted> руб`. Лучшее качество. |
| **Self-hosted** | Данные остаются в вашем контуре. Нужен OpenAI-compatible endpoint. Ответственность за данные — на пользователе. |

### Redact (маскирование значений)

В Cloud-режиме числовые значения заменяются на `<redacted>`:
- `price=10руб` → `price=<redacted> руб`
- `cac=120руб` → `cac=<redacted> руб`
- `months=36` → `months=<redacted>`

Единицы измерения сохраняются — LLM понимает контекст (рубли, проценты, месяцы).

### Где хранятся ключи

- **BYOK-ключи**: OS keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service)
- **OAuth-токены ChatGPT**: тоже локально (keychain desktop / OPFS encrypted web)
- **Бэкенд не участвует** — всё хранится на вашей машине

## Cost management

### Дневной лимит

По умолчанию **$1.00/день**. Настраивается в Settings → AI and Models (слайдер $0.25–$5.00).

- При **80% лимита** — диалог: "Расширить сегодняшний лимит? Изменить лимит по умолчанию?"
- При **100% лимита** — LLM-запросы отклоняются, Suggest работает на lex (без AI)

### Статусная панель

В правом нижнем углу (над миникартой) показывается:
- Активная модель и провайдер
- Включённые функции (toggle кликом)
- Cost за сессию (накопительный)
- Cost за день / дневной лимит
- Прогресс-бар (день cost / лимит)

На узких окнах (< 900px) панель скрывается.

### Стоимость моделей

| Модель | $/1000 запросов | p@1 (benchmark) |
|---|---|---|
| glm-5.3-flash (z.ai) | $0.121 | 1.000 |
| nemotron-super:free (OpenRouter) | $0 | 0.800 |
| jev-1.13 (System One) | $0.070 | 0.733 |
| Ollama (local) | $0 | ~0.5 |

## Suggest

### Как работает

1. Вы начинаете редактировать ноду
2. Через 500мс (debounce) отправляется запрос к LLM
3. LLM ранжирует шаблоны из каталога (19 опций)
4. Если confidence < порога (по умолчанию 0.5) — LLM генерирует 3 кастомных варианта
5. Подсказки показываются как ghost-ноды (полупрозрачные)

### Loading state

Пока LLM думает (0.5–10с), показывается skeleton ghost-node с spinner.
После ответа — skeleton morph'ит в реальную подсказку.

### Custom-ноды

Если ни один шаблон не подходит (confidence < threshold), LLM генерирует
3 варианта кастомных нод:
- Каждая с уникальным title + formula + params
- Формулы валидируются через `expr_eval` — битые варианты скрываются
- 3 ghost-nodes показываются рядом

### "Сохранить как шаблон"

После применения custom-ноды:
1. Toast: "Сохранить как шаблон? [Да] [Нет]"
2. "Да" → открывается GitHub issue (тип enhancement) к canvasdesk repo
3. Шаблон сохраняется локально в user-templates

## Graph Builder

### Как работает

1. Откройте Graph Builder (меню или `.byok`-файл)
2. Введите текст (например, meeting notes)
3. Выберите режим: mindmap / outline / summary
4. Нажмите "Сгенерировать"
5. Preview показывает граф (ghost-nodes)
6. Accept → вставка через `graph_apply` (один undo-шаг)

### Cost estimate

Перед запросом показывается оценка стоимости: "Запрос ~$0.02".

## Agent Panel

### Как работает

1. Откройте Agent Panel (Ctrl+I)
2. Напишите запрос: "создай ноду CAC и свяжи с LTV"
3. LLM вызывает MCP-инструменты (42 инструмента)
4. Preview показывает результат (ghost-nodes)
5. Accept → применение через `graph_apply`

### Selection-aware

- Если выбраны ноды → агент работает только с ними
- Если ничего не выбрано → контекст = весь канвас
- **Непросимые ноды не изменяются** — validation layer отклоняет операции

### Cost estimate

Перед запросом: "Запрос ~$0.02 · день $0.12 / $1.00"

## ChatGPT OAuth (Sign-in-with-ChatGPT)

### Как подключить

1. Settings → AI and Models
2. Выберите "ChatGPT OAuth" для Graph Builder или Agent Panel
3. Нажмите "Continue with ChatGPT"
4. Браузер откроет ChatGPT login
5. После login → access_token сохранён локально
6. Модель доступна

### Rate limits

ChatGPT Plus: ~80 сообщений / 3 часа для GPT-4o.
Статусная панель показывает остаток: "ChatGPT: 47/80 сообщений осталось".
При исчерпании → fallback на BYOK.

## Self-hosted endpoint

Для компаний с своим GPU-сервером (Ollama, vLLM, TGI, etc.):

1. Settings → AI and Models → Data residency: "Self-hosted"
2. Введите URL endpoint (например, `https://llm.corp.local/v1`)
3. Введите API key (если требуется)
4. Нажмите "Проверить" — health-check

**Важно:** ответственность за данные и безопасность endpoint — на пользователе.
CanvasDesk не проверяет SSL-сертификаты self-hosted endpoint'ов.

## Telemetry

По умолчанию **OFF**. При включении отправляются анонимные события:
- Показ подсказки
- Принятие/отклонение подсказки
- Cost запроса

Без PII (personal identifiable information). Полный гайд — в этом документе.

## Troubleshooting

### Suggest не работает

1. Проверьте Settings → AI and Models → Suggest: не "Off"
2. Проверьте API key (кнопка "Проверить ключ")
3. Проверьте дневной лимит (не исчерпан?)
4. Проверьте интернет (для cloud-режима)
5. Fallback: lex-подсказки работают всегда (без AI)

### Rate limit (429)

- ChatGPT OAuth: дождитесь обновления окна (3 часа) или переключитесь на BYOK
- BYOK: проверьте лимиты провайдера (OpenRouter dashboard)

### Agent Panel не выполняет запрос

1. Проверьте, что Agent Panel не "Off" в Settings
2. Проверьте, что выбран провайдер с `tool_calling` capability
3. Проверьте cost estimate (не превышен дневной лимит)

### Self-hosted endpoint не отвечает

1. Проверьте URL (должен быть `https://...` или `http://localhost:...`)
2. Проверьте, что endpoint OpenAI-compatible (`/v1/chat/completions`)
3. Проверьте API key (если требуется)
4. Проверьте SSL-сертификат (для HTTPS)

## Связанные документы

- [PRD-0010: LLM-интеграция](https://github.com/danku13/CanvasDesk/blob/main/docs/prd/prd-0010-llm-integration-byok-chatgpt.md)
- [ADR-0016: LLM-слой canvas-llm](https://github.com/danku13/CanvasDesk/blob/main/docs/adr/adr-0016-llm-layer-canvas-llm.md)
- [Benchmark 11 LLM](https://github.com/danku13/CanvasDesk/blob/main/docs/dev-researches/llm-mm-source-benchmark.md)
- [План 4 потоков](https://github.com/danku13/CanvasDesk/blob/main/docs/dev-researches/llm-implementation-plan-4-streams.md)
