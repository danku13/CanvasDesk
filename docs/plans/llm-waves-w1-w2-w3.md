# LLM-доработки: три волны внедрения (W1 → W2 → W3)

> **Назначение:** файл-задание для трёх агентских сессий. Одна волна = одна
> сессия агента со своим клоном репозитория. Волны спроектированы с
> **непересекающимися наборами файлов**, чтобы слияния в `main` проходили
> без конфликтов.
>
> **База:** `main` @ `1408f87` (2026-10-09). Контекст состояния wasm/ИИ:
> [WASM-AI-FEATURES.md](../WASM-AI-FEATURES.md) (§8 — сводка TODO-маркеров),
> [PRD-0010](../prd/prd-0010-llm-integration-byok-chatgpt.md),
> [ADR-0011](../adr/adr-0011-wasm-build-gate.md) (wasm-гейт),
> [ADR-0016](../adr/adr-0016-llm-layer-canvas-llm.md) (LLM-слой).
>
> **Поиск точек в коде** — по маркерам: `FR-LLM-D-TODO`, `FR-LLM-FIX-TODO`,
> `AI_OAUTH_UNAVAILABLE`, `OPFS — TODO`, `fetch-транспорт F-5.10`.
>
> **Статус исполнения** (исполнитель волны отмечает здесь):
> - Волна 1 — ✅ реализована (2026-10-09: HttpTransport/Ureq/Mock, wasm-fetch
>   за фичей, health::check + HealthReport, discovery + ModelCache TTL,
>   getrandom wasm_js-гейт; тесты 2736 workspace + 232 canvas-llm/features)
> - Волна 2 — ✅ реализована (2026-10-09: executor-сим LLM-футур, входы
>   панелей Ctrl+I/Graph Builder, реальные вызовы на нативе, cost, health-UI,
>   discovery-списки, OAuth web-сим, suggest mm-ранжирование в wasm)
> - Волна 3 — ✅ ядро реализовано (2026-10-09, сессия багфиксов владельца):
>   флип фич `l1-llm`+`wasm-fetch-bridge` в canvas-web (target-блок wasm32),
>   wasm-executor (spawn_local-шов), fetch-транспорт executor'а, платформенный
>   транспорт провайдеров (`llm_factory::platform_transport` — фикс паники
>   ureq «time not implemented» на wasm), OPFS token store + OAuth web-мост
>   (`llm_web.rs`: popup/callback/exchange, флаги конфига), бейдж ключа с
>   фактическим числом discovery + честный «ключ пустой» (репорт владельца).
>   Верифицировано на release-бандле: health OK + discovery 469 моделей,
>   агент-запрос POST /v1/chat/completions с честной ошибкой 401 на фейковом
>   ключе. Остаток: сквозной OAuth-прогон и PoC-отчёт (п.6–7) — после деплоя
>   `cloud/llm-proxy` владельцем; онбординг-сцена ai-mode (п.8) — следующая
>   правка.

---

## 0. Правила бесконфликтного слияния

1. **Порядок строгий:** W1 → мерж в `main` → W2 (ветка от свежего `main`) →
   мерж → W3 → мерж. Параллельно можно читать/анализировать, коммитить — только
   по порядку. Причина: W2 потребляет API волны 1, W3 — симы волны 2.
2. **Владение файлами** (§1): волна трогает только свои файлы. Чужие крейты и
   `docs/**` — запрещены. Найденный баг в чужом наборе — в отчёт волны, не в код.
3. **Внешние зависимости:** добавляет только W1 (внутри `crates/canvas-llm`,
   за feature). W2/W3 новых зависимостей не добавляют (wasm-bindgen/web-sys
   уже в дереве). `Cargo.lock` содержательно меняет только W1.
4. **Документация** (`docs/**`, `user-docs/**`, `README*`) — только W3.
   Волны 1–2 оформляют знания doc-комментариями; W3 сведёт их в доки.
5. **i18n** (`crates/canvas-app/src/i18n.rs`, ключи ru+en) — только W2.
6. **Гейты каждой волны** (обязательны перед отчётом, зелёные):

   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo check -p canvas-web --target wasm32-unknown-unknown   # гейт ADR-0011
   ```

   W3 дополнительно: `scripts/web_bundle.sh --release` локально; после пуша —
   зелёный CI `pages-web` и ручной прогон сценариев A–I из WASM-AI-FEATURES.md §6.
7. **Коммиты:** conventional (`feat(llm-core): …`, `feat(app-llm): …`,
   `feat(web-oauth): …`, `docs(llm): …`), только по своей волне.
8. **AGENTS.md, worklog-протокол сессии, CI-воркфлоу** (`.github/**`) — не трогать.

## 1. Матрица владения файлами

| Набор | W1 | W2 | W3 |
|---|---|---|---|
| `crates/canvas-llm/**` (src, tests, Cargo.toml) | ✅ владеет | читает | читает |
| `crates/canvas-app/**` | — | ✅ владеет | — |
| `crates/canvas-web/**` (вкл. `Cargo.toml`, `index.html`, `Trunk.toml`) | — | — | ✅ владеет |
| `cloud/llm-proxy/**` | — | — | ✅ владеет |
| `docs/**`, `user-docs/**` | — | — | ✅ владеет (кроме этого файла) |
| `scripts/**` (`web_bundle.sh`, `wasm_gate.sh`, новые `poc_*`) | — | — | ✅ владеет |
| `sdk/web-onboarding/**` | — | — | ✅ владеет |
| `crates/canvas-suggest/**`, `canvas-render/**`, `canvas-shell/**`, … | — | — | — (не трогать) |
| Корневые `Cargo.toml`/`Cargo.lock` | lock (авто) | — | — |

---

## 2. Волна 1 — «LLM-ядро: транспорт, health-check, discovery» (крейт `canvas-llm`)

**Цель.** Развязать сетевой слой от `ureq`, подготовить wasm-транспорт (браузерный
`fetch` через cloud-proxy, F-5.10), реализовать высокоуровневый health-check и
discovery моделей с кэшем. Всё — за существующими/новыми feature-флагами;
дефолтная (в т.ч. wasm) сборка остаётся zero-dep (ADR-0011 не нарушается).

**Задачи.**

1. **Транспортная абстракция** — новый `src/transport.rs`:
   - async-трейт `HttpTransport { async fn execute(Request) -> Response }`
     (async-trait уже в дереве за `l1-llm`); типы `Request { url, method,
     headers, body }`, `Response { status, headers, body }`;
   - реализация `UreqTransport` (натив; переносит текущие ureq-вызовы,
     таймаут 10 с сохраняется);
   - за feature `wasm-fetch` — `WasmFetchTransport` на `web-sys::fetch` +
     `JsFuture` (optional deps: web-sys/js-sys/wasm-bindgen-futures —
     **не** включать в default; реальное включение выполнит W3).
2. **Перевод всех сетевых точек на `HttpTransport`:** `openai_compat.rs`,
   `anthropic.rs`, `health.rs::check_endpoint`, `chatgpt_oauth/*`
   (authorize/token exchange/refresh, proxy pass-through в `provider.rs`).
   Прямых вызовов `ureq` вне `UreqTransport` остаться не должно.
   **Транспорт — единый шов для всех будущих вызовов моделей из wasm**
   (панели, suggest, health, discovery) — именно он делает полноценные
   LLM-вызовы из браузера возможными (через cloud-proxy, CORS).
3. **Health-check высокоуровнево:** `health::check(provider_config) -> HealthReport`
   (Ok / Auth / RateLimit{retry_after} / Transport с человекочитаемой деталью);
   сохранить текущую семантику статусов (401/403 → Auth, 429 → RateLimit).
   Это API для кнопок «Проверить» (подключит W2).
4. **Discovery моделей с кэшем** — новый `src/discovery.rs`:
   - `list_models(transport, base_url, auth) -> Result<Vec<ModelInfo>>`
     (GET `/v1/models`, парсинг JSON, сортировка, redact ошибок);
   - `ModelCache` in-memory с TTL 10 мин, ключ = (provider_id, base_url);
     persist между сессиями — сознательно в бэклог (см. §6).
5. **Тесты:** `MockTransport` для юнит-тестов провайдеров и OAuth pass-through
   (без сети); тесты TTL-кэша discovery; регресс `oauth_tests`, `provider_tests`,
   `cost_tests`, `redact_tests` зелёный.
6. **Проверка гейта ADR-0011:** `cargo check -p canvas-llm --target
   wasm32-unknown-unknown` (default) — zero-dep; с `--features l1-llm` —
   собирается без `wasm-fetch`; `l1-llm,wasm-fetch` — только под wasm32-целью.

**Запрещено:** трогать `canvas-app`/`canvas-web`/`docs`; включать `wasm-fetch`
в default; менять публичный трейт `LlmProvider` (методы/сигнатуры фиксированы).

**Критерий готовности:** все гейты зелёные; `rg "ureq::"` вне `transport.rs`
пуст; discovery+health покрыты тестами; в отчёте — коммиты, diffs API
(`HttpTransport`, `HealthReport`, `ModelInfo`, `ModelCache`), заметки для W2/W3.

---

## 3. Волна 2 — «Вживление в canvas-app: панели, входы, cost, health-UI, OAuth-сим»

**Цель.** Сделать ИИ-функции реально вызываемыми пользователем: подключить
точки входа панелей, заменить mock на реальные LLM-вызовы (натив), завести
реальные cost-цифры и health-check в UI, подготовить runtime-симы для
web-пути (их наполнит W3).

**Задачи.**

1. **Точки входа панелей:** подключить `agent_panel_hit` (`agent_panel.rs:1111`)
   и `graph_builder_hit` (`graph_builder_ui.rs:432`) к обработчику ввода в
   `app.rs`; хоткей Agent Panel (Ctrl+I по F-4 PRD), вход Graph Builder
   (кнопка/меню по F-3); подсказки/тосты через `i18n.rs` (ru+en).
   Это закрывает «панели недоступны пользователю» из WASM-AI-FEATURES §4.4.
2. **Реальные LLM-вызовы (натив):**
   - `agent_send` (`agent_panel.rs:935`) → `provider.tool_calling(...)` с
     обработкой tool-calls; после ответа — `graph_apply` батчем с undo-шагом
     (механизм FR-033); маркеры `FR-LLM-D-TODO` в `agent_panel.rs:883/892/933/1091`;
   - `graph_builder_generate` (`graph_builder_ui.rs:500`) → `GraphBuilder::build()`
     через фабрику; preview ghost-нод + Accept/Reject как сейчас;
     `graph_apply` с undo (`graph_builder_ui.rs:611`);
   - mock-поток сохранить как fallback при `NotSupported`/отсутствии сети
     (graceful degradation F-5.9) и для тестов.
3. **Cost-счётчики реальные:** после каждого успешного запроса
   `canvas_llm::cost::actual_cost(...)` → `ai_cost_session`/`ai_cost_day`
   (`app.rs:1287`); статусная панель показывает фактические цифры;
   дневной лимит-гейт уже работает — не регрессировать.
4. **Health-check UI:** кнопки «Проверить» (строка API-ключа и selfhost;
   маркеры `FR-LLM-FIX-TODO` в `overlays.rs:4839/5769`, `settings_ui.rs:1178`,
   `app.rs:1273`) → `canvas_llm::health::check` (API W1) в фоне
   (worker-поток на нативе); состояния: idle → проверяется → ok / ошибка
   (текст из `LlmError`), i18n; флаги `ai_key_ok`/`ai_selfhost_ok` из результата.
5. **Discovery-UI:** для BYOK/selfhost строк модели — выпадающий список/автодополнение
   из `canvas_llm::discovery::list_models` (кэш W1); при ошибке сети — fallback
   на свободный ввод (текущее поведение).
6. **OAuth web-сим (подготовка к W3):**
   - runtime-шов в `App` (поле + setter, default `None`): web-бридж OAuth
     (token store + запуск флоу), инъекцию выполнит `canvas-web` в W3;
   - `oauth_button_click` (`settings_ui.rs`): заменить компайл-тайм запрет
     (`cfg(any(not(feature = "l1-llm"), target_arch = "wasm32"))` → тост
     `AI_OAUTH_UNAVAILABLE`) на runtime-проверку: бриджа нет → прежний тост;
     бридж есть → делегировать флоу;
   - `oauth_assets()` wasm-ветка (`app.rs:9225-9231`): `token_store` из бриджа
     вместо жёсткого `None` (сейчас memory-store → Auth → F-5.9 fallback).
7. **Executor-сим для LLM-футур:** общий механизм запуска async-вызовов:
   натив — `std::thread` + канал (паттерны `suggest` worker / `oauth_flow`);
   wasm — заглушка, возвращающая понятное «недоступно до W3» (или callback-шим,
   который canvas-web наполнит `spawn_local`); поллинг результата — в
   `about_to_wait` (паттерн `oauth_poll`, `app.rs:9242`).
8. **Suggest: LLM mm-источник в wasm (частичный ИИ в web без панелей):**
   в 4 точках wasm-ветки `suggest::rank(..., None)` (`app.rs:6106/6112/6292/6298`)
   вместо жёсткого `None` — remote-источник `LlmMmSource` (API готово в
   canvas-suggest за `l1-llm`, сам крейт не трогаем) на провайдере из фабрики,
   исполнение через executor-сим (п. 7): мгновенно показываем lex-результат,
   LLM-ранжирование доезжает асинхронно (паттерн `suggest.pending`); ошибка /
   нет провайдера / пауза / лимит → вырождение в lex (`fusion(lex, ∅) = lex` —
   уже встроено). Cost LLM-запросов suggest — в `ai_cost_session/day` (п. 3).
   На нативе поведение не меняется (worker с remote-источником как сейчас).
   Генерация кастомных нод (F-2.7/F-2.12–F-2.16) — вне волн, бэклог (не
   реализована и на нативе, см. §6).
9. **Тесты:** юнит-тесты на входы панелей (hit → open), гейт паузы/лимита,
   cost-инкременты, health-состояния, discovery-fallback, OAuth-сим ветвление,
   suggest-деградация в lex (нет сети/провайдера/пауза). Регресс сценариев
   A–I (WASM-AI-FEATURES §6) вручную на `trunk serve`.

**Запрещено:** трогать `canvas-llm` (если не хватает мелочи в API W1 — в отчёт,
не в код), `canvas-web`, `docs/**`, добавлять внешние зависимости.

**Критерий готовности:** гейты зелёные; на нативе
(`cargo run -p canvas-app --features l1-llm-tls`) панели открываются, реальный
BYOK-запрос выполняется, cost растёт, «Проверить» даёт честный результат;
wasm-сборка ведёт себя как сейчас (без регресса сценариев A–I); в отчёте —
коммиты, описание симов (сеттеры/типы) для W3, найденные баги W1.

---

## 4. Волна 3 — «Web-путь F-5.10 + OPFS + документация + PoC»

**Цель.** Включить ИИ-функционал в web-сборке: OAuth ChatGPT и провайдеры через
cloud-proxy (fetch-транспорт W1 + бриджи W2), OPFS-токен-стор; привести
документацию к послеволновому состоянию; собрать PoC-валидацию по критериям
PRD-0010.

**Задачи.**

1. **OPFS Token Store:** расширить `crates/canvas-web/src/opfs.rs` — реализация
   token-store трейта canvas-llm (сим `OAuthAssets.token_store` из W2):
   запись `oauth_tokens.json`-эквивалента в OPFS (origin-scoped), чтение при
   старте, удаление при выходе; инъекция в `App` из `app_spawn.rs`.
2. **Wasm-executor:** наполнить сим W2 — `spawn_local` + пробуждение
   `about_to_wait` (паттерн `oauth_poll`/`suggest.pending`).
3. **Feature-флип canvas-web:** `crates/canvas-web/Cargo.toml` →
   `canvas-app = { features = ["l1-llm"] }`, `canvas-llm` c `wasm-fetch`;
   поправить `wasm_gate.sh`/доки, если гейт предполагал полное отсутствие сети —
   теперь сеть только через `WasmFetchTransport` (браузерный fetch, CORS
   cloud-proxy). Правку ADR-0011 см. п. 5. Флип активирует и suggest
   mm-источник (W2 п. 8) — LLM-ранжирование подсказок в браузере.
4. **OAuth web-флоу (F-5.10):** `js_glue.rs`/`index.html` — детект
   `?oauth_callback=` (редирект 302 воркера) / `canvasdesk://oauth/callback`
   deep-link; открытие popup логина на authorize-URL (через деплой прокси);
   exchange code через pass-through прокси (`WasmFetchTransport`); сохранение
   токенов в OPFS-стор; обновление бейджа 9-го таба; тосты ошибок i18n-ключами
   существующими (W2)/новыми web-ключами (в `i18n.rs` не лезть — строки через
   бридж/JS-сторону либо согласовать в отчёте).
5. **Документация (сводная, после волн 1–2):**
   - `docs/WASM-AI-FEATURES.md` — обновить матрицу §1 (в т.ч. строка Suggest —
     LLM-ранжирование в web, строки панелей/OAuth/health), снять актуальные
     заглушки §4.6/4.7/§8, добавить сценарии проверки web-OAuth, панелей и
     suggest-ранжирования в web;
   - `user-docs/ai-features.md` — раздел web (вход ChatGPT в браузере, BYOK
     через прокси, health-check, discovery моделей);
   - `docs/adr/adr-0011-wasm-build-gate.md` — amendment: сеть в wasm разрешена
     за явной feature, транспорт — браузерный fetch только к деплоенному
     `cloud/llm-proxy` (CORS), дефолтная сборка остаётся без сети;
   - `docs/prd/prd-0010-…` — статусы F-3/F-4/F-5.10/F-7 (что закрыто волнами);
   - `docs/index.md`, `docs/BYOK.md`, `README*` — точечные правки ссылок/статусов.
6. **cloud/llm-proxy:** ревизия README + пошаговый ранбук деплоя
   (`wrangler deploy`, секрет `PROXY_BYOK_KEY`, кастом-домен/redirect-URL
   регистрации OAuth-клиента); smoke-скрипт проверки живого деплоя
   (`scripts/poc_proxy_smoke.sh`); правки воркера — только в пределах `cloud/llm-proxy/**`.
7. **PoC-харнесс (критерии PRD-0010):** `scripts/poc_llm_*.sh|py` +
   `docs/dev-researches/poc-llm-report.md`:
   - setup ≤ 30 с: от чистого профиля до первого успешного health-check (BYOK);
   - p@1 ≥ 0.8 suggest-ранжира на evalset (`canvas-suggest/tests/fixtures/evalset`);
   - cost-замеры: tokens × цены `cost.rs` ≤ дневного бюджета на сценарий.
8. **Онбординг-тур:** обновить `sdk/web-onboarding/src/scenarios/ai-mode.ts`
   (шаг про вход ChatGPT в web теперь активен, ссылки на обновлённый гайд).
9. **Финальные проверки:** гейты; локальный release-бандл (`scripts/web_bundle.sh
   --release`); после пуша — зелёный CI `pages-web`; ручной прогон сценариев
   A–I + новых web-OAuth сценариев на https://danku13.github.io/CanvasDesk/app/.

**Запрещено:** трогать `crates/canvas-llm/**` и `crates/canvas-app/**` (баги —
в отчёт владельцу волны), добавлять внешние зависимости в Rust-крейты.

**Внешняя зависимость (владелец, не агент):** деплой воркера должен быть
выполнен ДО п.4/9: `cd cloud/llm-proxy && wrangler deploy` (+ секрет).
Без него web-OAuth тестируется только с локальным `wrangler dev`.

**Критерий готовности:** в web-сборке выполняется вход ChatGPT (токены в OPFS,
бейдж Connected), BYOK-запрос через прокси отвечает, health-check в web
работает; доки отражают фактическое состояние; PoC-отчёт с цифрами по трём
критериям; CI зелёный; в отчёте — коммиты, ссылка на прод-стенд, результаты PoC.

---

## 5. Порядок интеграции и чек-лист владельца

```text
W1 (canvas-llm) ──мерж──> W2 (canvas-app) ──мерж──> W3 (canvas-web+docs+PoC) ──мерж──> main
```

- После каждого мержа: гейты §0.6 + smoke нативной сборки
  (`cargo run -p canvas-app --features l1-llm-tls`).
- Конфликты не ожидаются (наборы файлов не пересекаются); единственный
  потенциальный файл — `Cargo.lock`, его содержательно меняет только W1.
- Каждая волна обязана дописать раздел «Статус исполнения» в шапке этого файла
  (только свои строки) и приложить отчёт в описание PR/коммит-сообщения.

**Чек-лист владельца (не агентские задачи):**
- [ ] **Ротация GitHub PAT** — токен засвечен в чате (срочно).
- [ ] Деплой `cloud/llm-proxy`: `wrangler deploy` + секрет `PROXY_BYOK_KEY`
      (нужно для W3 п.4/9; можно параллельно с W1/W2).
- [ ] Живой OAuth-прогон ChatGPT на desktop после W2.
- [ ] Приёмка PoC-отчёта W3 по критериям PRD-0010.

## 6. Вне скоупа волн (бэклог, зафиксирован сознательно)

- Web-worker для suggest на wasm (FR-064) — перф-оптимизация, текущий
  sync-lex на UI-треде работает < 1 мс на каталоге 62 опций.
- Шифрование BYOK-ключа в localStorage (WebCrypto) — hardening после волн.
- Адаптив статусной AI-панели для окон < 900px (сейчас скрывается by design).
- Persist discovery-кэша моделей между сессиями (W1 делает in-memory).
- Генерация кастомных вариантов нод LLM (F-2.7, F-2.12–F-2.16) — не реализована
  ни на одной платформе (вне рамок Stream C этап 1, `custom_suggest.rs` — TODO
  плана 4-streams); в волны не входит, отдельная задача после W3.
- MCP-агентная композиция (ADR-0004/0009/0010/0012) — отдельный трек, в рамку
  этих волн не входит.
