# canvasdesk-llm-proxy — Cloudflare Worker: OAuth-callback + CORS-прокси для wasm

- **Статус:** реализовано (v1), stateless
- **Тип:** инфраструктура (`cloud/llm-proxy/`, чистый ESM-JavaScript, без сборки и npm-зависимостей)
- **Требования:** PRD-0010 `docs/prd/prd-0010-llm-integration-byok-chatgpt.md` — **F-5.10** («Cloud function для wasm ТОЛЬКО для OAuth-callback redirect, НЕ хранит токены»), **Q6** («всё локально, бэкенд не нужен»), F-6.1–F-6.4 (web proxy для wasm); маркер `FR-LLM-PROXY`
- **Связанные документы:** `docs/dev-researches/byok-chatgpt-oauth-design.md` (§4 Sign-in-with-ChatGPT flow, §6 риски/безопасность), `docs/adr/adr-0011-wasm-build-gate.md` (wasm-гейт), `docs/adr/adr-0016-llm-layer-canvas-llm.md` (canvas-llm)
- **Создан:** 2026-10-04

---

## 1. Назначение

Wasm-сборка CanvasDesk (`crates/canvas-web/`) не может:

1. принять OAuth-redirect на `localhost` — redirect_uri обязан быть HTTPS
   (дизайн-док §4.7, п.4 «Desktop-first»);
2. держать API-ключи в bundle — публичный wasm читается кем угодно (риск §6.1);
3. ходить на `api.openai.com` без CORS.

Этот воркер закрывает все три пункта и при этом **ничего не хранит**: ни
токенов, ни кодов, ни состояния (Q6). Токены пользователя живут только в
клиенте (desktop — OS keychain, web — OPFS encrypted, НЕ localStorage).

## 2. Архитектура потоков

```
                         ┌────────────────────────────────────┐
                         │  cloud/llm-proxy (этот воркер)     │
                         │  stateless: нет KV/DO/R2/D1/cookies│
                         └────────────────────────────────────┘

Desktop (натива) — прокси НЕ нужен:
  браузер ─ login ─► chatgpt.com/auth/login ─ redirect ─► 127.0.0.1:PORT/callback (listener)
  canvas-app ── POST auth.openai.com/oauth/token (PKCE, напрямую) ──► keychain

Web (wasm):
  1) login      браузер ─► chatgpt.com/auth/login   (redirect_uri = https://<proxy>/oauth/callback)
                воркер: GET /oauth/callback?code&state
                 └─ 302 → canvasdesk://oauth/callback?code&state  (deep-link, F-5.2/F-5.10)
                    ОС открывает приложение, code/state забирает клиент

  2) exchange   canvas-web ─► POST https://<proxy>/oauth/token ─► auth.openai.com/oauth/token
                (pass-through: тело как есть; access/refresh token возвращаются
                 клиенту и НЕ сохраняются воркером)

  3) inference  canvas-web ─► GET/POST https://<proxy>/v1/models|responses ─► api.openai.com
                Authorization: Bearer <oauth access_token> — как есть
                (нет заголовка + секрет PROXY_BYOK_KEY → Bearer PROXY_BYOK_KEY,
                 режим BYOK-through-proxy: ключ в env воркера, не в wasm-bundle)
```

## 3. Эндпоинты

| Метод | Путь | Назначение | Вход → Выход |
|---|---|---|---|
| GET | `/oauth/callback` | OAuth-callback redirect (F-5.10) | `?code=..&state=..` (+ любые доп. параметры OpenAI) → **302** `Location: canvasdesk://oauth/callback?code=..&state=..`; нет `code`/`state` → **400**. Ничего не пишет/не хранит/не логирует |
| POST | `/oauth/token` | Stateless pass-through token exchange (PKCE) для wasm (Q6) | JSON **или** urlencoded `{grant_type, code, code_verifier, client_id, redirect_uri, resource}` (или `{grant_type: refresh_token, refresh_token, client_id}`) → пересылается **как есть** на `https://auth.openai.com/oauth/token`; ответ upstream (статус + тело + Content-Type) — как есть клиенту. Мусорные/чужие гранты → **400** |
| GET | `/v1/models` | Model discovery (F-5.5) | `Authorization: Bearer <access_token>` (как есть) → ответ `api.openai.com/v1/models`. Нет заголовка и `PROXY_BYOK_KEY` → **401** |
| POST | `/v1/responses` | Inference, OpenAI Responses API (F-5.6) | Тело как есть + `Authorization` как есть → ответ upstream как есть (тело пробрасывается потоком, Content-Type сохраняется). Лимит тела 1 МБ → **413** |
| OPTIONS | `*` | CORS preflight (обязателен для wasm origin) | → **204** + `Access-Control-Allow-*` (`Allow-Origin` из `ALLOWED_ORIGIN`, по умолчанию `*`; методы `GET, POST, OPTIONS`; заголовки `Authorization, Content-Type`; `Max-Age 86400`) |
| любой | прочие пути | — | **404**; верный путь, неверный метод → **405** (+ `Allow`) |

Все ответы несут `X-Content-Type-Options: nosniff`; ответы `/oauth/token`,
`/v1/*` и redirect — `Cache-Control: no-store`.

## 4. Деплой

```bash
npm i -g wrangler
wrangler deploy                      # из cloud/llm-proxy/ (берёт wrangler.toml)
wrangler secret put PROXY_BYOK_KEY   # опционально: режим BYOK-through-proxy
```

Перед прод-деплоем:

1. `ALLOWED_ORIGIN` в `wrangler.toml [vars]` → конкретный origin wasm-приложения
   (например `"https://canvasdesk.dev"`), не `"*"`;
2. `redirect_uri` в OAuth-запросе клиента = `https://<workers-domain>/oauth/callback`
   — тот же хост, что задеплоен;
3. если установленный wrangler/workerd старше `compatibility_date` — понизьте
   дату (wrangler подскажет минимально допустимую).

## 5. Переменные окружения

| Переменная | Тип | По умолчанию | Назначение |
|---|---|---|---|
| `ALLOWED_ORIGIN` | var | `*` | Значение `Access-Control-Allow-Origin`. В проде — конкретный origin wasm |
| `RATE_LIMIT_PER_MIN` | var | `120` | Запросов в минуту на IP (best-effort, окно 60 с) |
| `OAUTH_DEEP_LINK_BASE` | var | `canvasdesk://oauth/callback` | Схема/путь deep-link для OAuth-callback (константы вынесены в конфиг) |
| `MAX_BODY_BYTES` | var | `1048576` | Лимит тела запроса (1 МБ) → 413 |
| `PROXY_BYOK_KEY` | **secret** | — | API-ключ прокси для режима BYOK-through-proxy (`wrangler secret put`, НЕ в toml). Если задан — запросы `/v1/*` без `Authorization` идут под `Bearer PROXY_BYOK_KEY` |

## 6. Безопасность

- **Не хранится ничего** (Q6/F-5.10): нет KV/Durable Objects/R2/D1, cookies,
  сессий; `code`/`state`/`access_token`/`refresh_token` живут в памяти изоляции
  только на время обработки запроса. Tokens клиента — OPFS encrypted на web
  (NF-1), keychain на desktop.
- **Логирование:** access_token/refresh_token/code/code_verifier НЕ логируются
  никогда — ни в консоль, ни в тексты ошибок (тела запросов не логируются вовсе;
  в единственном месте console.error URL прогоняется через
  `redactQueryForLog()` — чувствительные параметры заменяются на `<redacted>`).
- **Тела:** ≤ 1 МБ (`MAX_BODY_BYTES`), превышение → 413; `/oauth/token`
  принимает только гранты `authorization_code`/`refresh_token` с обязательными
  полями (`validateTokenBody`) — прокси не открытый релей для произвольных OAuth-грантов.
- **Рейт-лимитер (best-effort):** in-memory `Map` по IP (CF-Connecting-IP),
  фиксированное окно 60 с, дефолт 120 req/min (`RATE_LIMIT_PER_MIN`).
  **Ограничение:** состояние живёт внутри одного isolate Workers — счётчики не
  разделяются между изоляциями и сбрасываются при рестарте; «unknown» IP (без
  CF-заголовков) попадает в один общий бакет.
  **TODO (прод):** перенести на Cloudflare KV/Durable Objects или WAF rate
  limiting; добавить audit log (F-6.3) без чувствительных данных.
- **Заголовки:** `nosniff` на всех ответах, `no-store` на токен-путях,
  `Referrer-Policy: no-referrer` на redirect (чтобы `code` не утёк в Referer).
- CORS: в проде `ALLOWED_ORIGIN` = конкретный origin (см. §4). Возможность
  списка origin'ов — TODO.

## 7. Тесты

```bash
npm test          # = node --test (рекурсивно находит test/)
node --test       # то же, из корня cloud/llm-proxy/
```

45 тестов (node:test, без wrangler и без сети): callback redirect (сохранение
query-параметров, отсутствие code/state), validateTokenBody (гранты, мусор),
parseTokenBody (JSON/urlencoded), рейт-лимитер с **инъекцией часов** (окно,
retryAfter, сброс после окна — без setTimeout), corsHeaders/securityHeaders,
pickAuthorization (BYOK-through-proxy), redactQueryForLog, и роутер
`handleRequest` c подменой `globalThis.fetch`: pass-through token exchange,
проброс Authorization, 401/404/405/413/429.

## 8. Связь с Rust-стороной

- `crates/canvas-llm/src/chatgpt_oauth/` (Stream D): `proxy_url` = base URL
  воркера; token exchange → `{base}/oauth/token`; inference → `{base}/v1/responses`;
  model discovery → `{base}/v1/models`. В wasm вместо ureq — browser `fetch`.
- `crates/canvas-web/src/llm_proxy.rs` (Stream D) — клиентская обвязка fetch к
  этому прокси; док-комментарий `crates/canvas-llm/src/lib.rs`: «Web (wasm):
  cloud-proxy … держит API-ключи в env, canvas-llm делает fetch к proxy».
- Desktop-сборка прокси НЕ использует (localhost-listener + прямой token
  exchange, дизайн-док §4.2).

## 9. Ограничения и TODO

- Рейт-лимитер per-isolate → KV/DO/WAF (см. §6), audit log (F-6.3).
- Стриминг SSE `/v1/responses` в v1 не требуется, но тело upstream пробрасывается
  потоком (`Response(upstream.body)`) — Content-Type сохраняется, SSE пройдёт.
- `ALLOWED_ORIGIN` — одно значение; список origin'ов / динамическая проверка — TODO.
- id_token verification (JWKS) — на клиенте (`canvas-llm`), не на прокси: прокси
  сознательно остаётся «глупым» транзитом (Q6).

## 10. История

- `2026-10-04` — агент (Task 2-b): воркер создан. GET /oauth/callback (302
  deep-link, F-5.10), POST /oauth/token (stateless pass-through, Q6), GET
  /v1/models + POST /v1/responses (CORS-прокси, BYOK-through-proxy через
  PROXY_BYOK_KEY), OPTIONS preflight, in-memory рейт-лимит 120 req/min, лимит
  тела 1 МБ, nosniff/no-store, redactQueryForLog. 45 тестов node:test зелёные.
