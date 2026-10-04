/**
 * canvasdesk-llm-proxy — stateless Cloudflare Worker для wasm-сборки CanvasDesk.
 *
 * Назначение (docs/prd/prd-0010-llm-integration-byok-chatgpt.md):
 *  - F-5.10: cloud function для wasm ТОЛЬКО для OAuth-callback redirect — токены
 *    НЕ хранятся (Q6: «всё локально, бэкенд не нужен»). Wasm-сборка не может
 *    принять redirect на localhost, поэтому OpenAI шлёт code/state сюда, а
 *    воркер делает 302 на deep-link `canvasdesk://oauth/callback?code=..&state=..`.
 *  - POST /oauth/token — stateless pass-through token exchange (PKCE) на
 *    https://auth.openai.com/oauth/token (дизайн-док §4.2, шаг 4): тело
 *    пересылается как есть, ответ — как есть, ничего не пишется и не логируется.
 *  - GET /v1/models + POST /v1/responses — CORS-прокси к https://api.openai.com:
 *    Authorization пользователя (OAuth access_token) пробрасывается как есть;
 *    опционально, если заголовка нет и задан секрет PROXY_BYOK_KEY, подставляется
 *    `Bearer PROXY_BYOK_KEY` (режим BYOK-through-proxy — см. док-комментарий
 *    crates/canvas-llm/src/lib.rs: «cloud-proxy держит API-ключи в env,
 *    canvas-llm делает fetch к proxy»).
 *
 * Связанные документы: ADR-0011 (wasm-гейт), ADR-0016 (canvas-llm LLM-слой),
 * docs/dev-researches/byok-chatgpt-oauth-design.md (§4 Sign-in-with-ChatGPT,
 * §6 риски/безопасность). Маркер: FR-LLM-PROXY (grep по репо).
 *
 * Безопасность (дизайн-док §6.1):
 *  - НИКОГДА не логируем access_token/refresh_token/code/code_verifier — ни в
 *    консоль, ни в тексты ошибок (см. redactQueryForLog + отсутствие логов тел).
 *  - Все ответы несут X-Content-Type-Options: nosniff; токен-ответы — no-store.
 *  - Тело запроса не более 1 МБ (413 при превышении).
 *  - 404 на неизвестные пути, 405 на неверные методы.
 *
 * Экспортируемые чистые функции (тестируются `node --test test/` без wrangler):
 *   buildCallbackRedirect, corsHeaders, securityHeaders, validateTokenBody,
 *   parseTokenBody, createRateLimiter, pickAuthorization, redactQueryForLog,
 *   clientIp; роутер — handleRequest(request, env, limiter?); дефолтный
 *   экспорт — fetch-хендлер для Workers.
 */

// FR-LLM-PROXY: маркер для поиска (grep): все файлы прокси помечены им.

// ---------------------------------------------------------------------------
// Конфигурация: константы эндпоинтов и лимитов. Переопределения через env —
// см. wrangler.toml [vars] и README «Переменные окружения».
// ---------------------------------------------------------------------------

/** Хост OpenAI API (Responses API + model discovery). */
export const OPENAI_API_BASE = 'https://api.openai.com';

/** OAuth token endpoint (дизайн-док §4.2, шаг 4). */
export const OPENAI_TOKEN_ENDPOINT = 'https://auth.openai.com/oauth/token';

/** Deep-link схема/путь для OAuth-callback в wasm (F-5.2/F-5.10). */
export const DEFAULT_DEEP_LINK_BASE = 'canvasdesk://oauth/callback';

/** Путь OAuth-callback на воркере (redirect-only). */
export const CALLBACK_PATH = '/oauth/callback';

/** Путь stateless token exchange (pass-through). */
export const TOKEN_PATH = '/oauth/token';

/** Путь inference-прокси (OpenAI Responses API, F-5.6). */
export const RESPONSES_PATH = '/v1/responses';

/** Путь model discovery (F-5.5). */
export const MODELS_PATH = '/v1/models';

/** Дефолтный рейт-лимит: запросов в минуту на IP. */
export const DEFAULT_RATE_LIMIT_PER_MIN = 120;

/** Окно рейт-лимитера (фиксированное, 60 секунд). */
export const RATE_LIMIT_WINDOW_MS = 60_000;

/** Защита in-memory Map лимитера от переполнения (isolate-local). */
export const RATE_LIMIT_MAX_ENTRIES = 10_000;

/** Максимальный размер тела запроса по умолчанию (1 МБ). */
export const DEFAULT_MAX_BODY_BYTES = 1_048_576;

const CORS_MAX_AGE_SEC = '86400';
const REDACTED = '<redacted>';
// Параметры, значение которых никогда не попадает в логи (см. redactQueryForLog).
const SENSITIVE_PARAMS = new Set([
  'code',
  'state',
  'access_token',
  'refresh_token',
  'id_token',
  'code_verifier',
]);
const TEXT_ENCODER = new TextEncoder();

// ---------------------------------------------------------------------------
// Чистые функции (без fetch/Request) — покрыты test/proxy.test.js
// ---------------------------------------------------------------------------

/**
 * CORS-заголовки. Origin берётся из env.ALLOWED_ORIGIN (по умолчанию «*»);
 * в проде рекомендуется конкретный origin wasm-приложения.
 * @param {{ALLOWED_ORIGIN?: string}} [env]
 */
export function corsHeaders(env = {}) {
  const origin = (env.ALLOWED_ORIGIN && String(env.ALLOWED_ORIGIN).trim()) || '*';
  return {
    'access-control-allow-origin': origin,
    'access-control-allow-methods': 'GET, POST, OPTIONS',
    'access-control-allow-headers': 'Authorization, Content-Type',
    'access-control-max-age': CORS_MAX_AGE_SEC,
  };
}

/** Обязательный security-заголовок для всех ответов прокси. */
export function securityHeaders() {
  return { 'x-content-type-options': 'nosniff' };
}

/**
 * GET /oauth/callback?code=..&state=.. → URL deep-link с сохранёнными
 * query-параметрами (F-5.10). Возвращает null, если code/state отсутствуют
 * или requestUrl невалиден (вызывающий код отвечает 400).
 * @param {string} requestUrl полный URL входящего запроса
 * @param {string} [deepLinkBase] база deep-link (по умолчанию canvasdesk://oauth/callback)
 * @returns {string|null}
 */
export function buildCallbackRedirect(requestUrl, deepLinkBase = DEFAULT_DEEP_LINK_BASE) {
  if (!deepLinkBase) return null;
  let url;
  try {
    url = new URL(requestUrl);
  } catch {
    return null;
  }
  const code = url.searchParams.get('code');
  const state = url.searchParams.get('state');
  if (!code || !state) return null;
  // Все query-параметры сохраняются (OpenAI может добавить scope и др.).
  // Конкатенация строк вместо new URL(deepLinkBase): не-special-схемы
  // (canvasdesk://) парсятся нестандартно, а конкатенация — детерминирована.
  const query = url.searchParams.toString();
  const sep = deepLinkBase.includes('?') ? '&' : '?';
  return query ? `${deepLinkBase}${sep}${query}` : deepLinkBase;
}

const isNonEmpty = (v) => typeof v === 'string' && v.trim().length > 0;

/**
 * Валидация тела token exchange. Разрешены только гранты PKCE-flow:
 *  - authorization_code: code, code_verifier, redirect_uri, client_id;
 *  - refresh_token: refresh_token, client_id.
 * Всё остальное (в т.ч. client_credentials/password) — false: прокси не
 * должен быть открытым релеем для произвольных OAuth-грантов.
 * @param {Record<string, unknown>|null} body
 * @returns {boolean}
 */
export function validateTokenBody(body) {
  if (body === null || typeof body !== 'object' || Array.isArray(body)) return false;
  if (body.grant_type === 'authorization_code') {
    return (
      isNonEmpty(body.code) &&
      isNonEmpty(body.code_verifier) &&
      isNonEmpty(body.redirect_uri) &&
      isNonEmpty(body.client_id)
    );
  }
  if (body.grant_type === 'refresh_token') {
    return isNonEmpty(body.refresh_token) && isNonEmpty(body.client_id);
  }
  return false;
}

/**
 * Разбор тела token exchange: JSON или urlencoded (OpenAI шлёт form).
 * Неизвестный content-type или битое тело → null (вызывающий код ответит 400).
 * @param {string} rawBody
 * @param {string} contentType
 * @returns {Record<string, string>|null}
 */
export function parseTokenBody(rawBody, contentType) {
  const ct = String(contentType || '').toLowerCase();
  try {
    if (ct.includes('application/json')) return JSON.parse(rawBody);
    if (ct.includes('application/x-www-form-urlencoded') || ct === '') {
      return Object.fromEntries(new URLSearchParams(rawBody));
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * Рейт-лимитер (best-effort, isolate-local): фиксированное окно windowMs,
 * счётчик на ключ (IP) в Map. Время инъецируется через `now` — тесты подменяют
 * часы без setTimeout. Ограничение: состояние живёт внутри одного isolate
 * Workers (см. README «Безопасность» — TODO KV/DO для прода).
 * @param {{limit?: number, windowMs?: number, now?: () => number, maxEntries?: number}} [opts]
 */
export function createRateLimiter({
  limit = DEFAULT_RATE_LIMIT_PER_MIN,
  windowMs = RATE_LIMIT_WINDOW_MS,
  now = () => Date.now(),
  maxEntries = RATE_LIMIT_MAX_ENTRIES,
} = {}) {
  const hits = new Map(); // key -> { count, windowStart }
  const sweep = (t) => {
    for (const [key, entry] of hits) {
      if (t - entry.windowStart >= windowMs) hits.delete(key);
    }
  };
  return {
    /**
     * @returns {{allowed: boolean, remaining: number, retryAfterSec: number}}
     */
    check(key) {
      const t = now();
      const entry = hits.get(key);
      if (!entry || t - entry.windowStart >= windowMs) {
        if (hits.size >= maxEntries) {
          sweep(t);
          if (hits.size >= maxEntries) hits.clear(); // best-effort защита памяти
        }
        hits.set(key, { count: 1, windowStart: t });
        return { allowed: true, remaining: Math.max(0, limit - 1), retryAfterSec: 0 };
      }
      if (entry.count >= limit) {
        const retryAfterSec = Math.max(1, Math.ceil((entry.windowStart + windowMs - t) / 1000));
        return { allowed: false, remaining: 0, retryAfterSec };
      }
      entry.count += 1;
      return { allowed: true, remaining: Math.max(0, limit - entry.count), retryAfterSec: 0 };
    },
    /** Полный сброс (для тестов). */
    reset() {
      hits.clear();
    },
    /** Число отслеживаемых ключей (для тестов/диагностики). */
    size() {
      return hits.size;
    },
  };
}

/**
 * Авторизация для /v1/*: Authorization пользователя как есть; если заголовка
 * нет и настроен PROXY_BYOK_KEY — `Bearer <PROXY_BYOK_KEY>` (BYOK-through-proxy).
 * Иначе null → 401.
 * @param {Request} request
 * @param {{PROXY_BYOK_KEY?: string}} [env]
 * @returns {string|null}
 */
export function pickAuthorization(request, env = {}) {
  const incoming = request.headers.get('authorization');
  if (incoming && incoming.trim().length > 0) return incoming.trim();
  const proxyKey = env.PROXY_BYOK_KEY ? String(env.PROXY_BYOK_KEY).trim() : '';
  if (proxyKey) return `Bearer ${proxyKey}`;
  return null;
}

/**
 * IP клиента: Cloudflare CF-Connecting-IP, иначе первый X-Forwarded-For,
 * иначе 'unknown' (все unknown попадают в один «виртуальный» бакет лимитера).
 * @param {Request} request
 * @returns {string}
 */
export function clientIp(request) {
  const cf = request.headers.get('cf-connecting-ip');
  if (cf && cf.trim()) return cf.trim();
  const xff = request.headers.get('x-forwarded-for');
  if (xff) {
    const first = xff.split(',')[0];
    if (first && first.trim()) return first.trim();
  }
  return 'unknown';
}

/**
 * Редактирование чувствительных query-параметров для ЛЮБОЙ будущей лог-строки
 * (code/state/access_token/refresh_token/id_token/code_verifier → '<redacted>').
 * Прокси сам ничего чувствительного не логирует — это страховка.
 * @param {string} urlString
 * @returns {string}
 */
export function redactQueryForLog(urlString) {
  try {
    const url = new URL(urlString);
    for (const key of [...url.searchParams.keys()]) {
      if (SENSITIVE_PARAMS.has(key.toLowerCase())) url.searchParams.set(key, REDACTED);
    }
    return url.toString();
  } catch {
    return '<unparsable-url>';
  }
}

// ---------------------------------------------------------------------------
// Внутренние хелперы ответов
// ---------------------------------------------------------------------------

function parseIntEnv(env, key, fallback) {
  const raw = env ? env[key] : undefined;
  if (raw === undefined || raw === null || String(raw).trim() === '') return fallback;
  const n = Number.parseInt(String(raw), 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

function resolveBodyLimitBytes(env) {
  return parseIntEnv(env, 'MAX_BODY_BYTES', DEFAULT_MAX_BODY_BYTES);
}

function resolveDeepLinkBase(env) {
  const fromEnv = env && env.OAUTH_DEEP_LINK_BASE ? String(env.OAUTH_DEEP_LINK_BASE).trim() : '';
  return fromEnv || DEFAULT_DEEP_LINK_BASE;
}

function jsonErrorResponse(status, code, message, env, extraHeaders = {}) {
  return new Response(JSON.stringify({ error: { code, message } }), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      ...corsHeaders(env),
      ...securityHeaders(),
      'cache-control': 'no-store',
      ...extraHeaders,
    },
  });
}

/**
 * Проброс ответа upstream как есть: статус, поток тела (стриминг SSE тоже
 * пройдёт), Content-Type сохраняется; добавляются CORS + nosniff + no-store.
 * Тело/токены не читаются и не логируются.
 */
function passThroughResponse(upstream, env) {
  const headers = { ...corsHeaders(env), ...securityHeaders(), 'cache-control': 'no-store' };
  const contentType = upstream.headers.get('content-type');
  if (contentType) headers['content-type'] = contentType;
  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers,
  });
}

/** Читает тело запроса с лимитом (по Content-Length и фактически). null → 413. */
async function readBodyWithinLimit(request, env) {
  const limit = resolveBodyLimitBytes(env);
  const declared = Number(request.headers.get('content-length') || '0');
  if (Number.isFinite(declared) && declared > limit) return null;
  const raw = await request.text();
  if (TEXT_ENCODER.encode(raw).length > limit) return null;
  return raw;
}

// ---------------------------------------------------------------------------
// Обработчики эндпоинтов
// ---------------------------------------------------------------------------

/** GET /oauth/callback — F-5.10: redirect-only, ничего не пишет и не хранит. */
function handleCallback(url, env) {
  const target = buildCallbackRedirect(url.toString(), resolveDeepLinkBase(env));
  if (!target) {
    // code/state отсутствуют → 400. Значения параметров НЕ эхоим и НЕ логируем.
    return jsonErrorResponse(
      400,
      'missing_code_or_state',
      'Ожидается GET /oauth/callback?code=..&state=.. (redirect от auth.openai.com).',
      env,
    );
  }
  return new Response(null, {
    status: 302,
    headers: {
      location: target,
      ...securityHeaders(),
      'cache-control': 'no-store',
      'referrer-policy': 'no-referrer',
    },
  });
}

/** POST /oauth/token — stateless pass-through token exchange (PKCE). */
async function handleTokenExchange(request, env) {
  const raw = await readBodyWithinLimit(request, env);
  if (raw === null) {
    return jsonErrorResponse(
      413,
      'payload_too_large',
      `Тело запроса больше ${resolveBodyLimitBytes(env)} байт.`,
      env,
    );
  }
  const contentType = request.headers.get('content-type') || '';
  const body = parseTokenBody(raw, contentType);
  if (!validateTokenBody(body)) {
    // Тело НЕ логируем (в нём code/code_verifier).
    return jsonErrorResponse(
      400,
      'invalid_token_request',
      'Ожидается grant_type=authorization_code (code, code_verifier, redirect_uri, client_id) или refresh_token (refresh_token, client_id).',
      env,
    );
  }
  // Pass-through: тело как есть, Content-Type сохраняется, ответ как есть.
  // access_token/refresh_token проходят транзитом — не хранятся и не логируются.
  const upstream = await fetch(OPENAI_TOKEN_ENDPOINT, {
    method: 'POST',
    headers: {
      'content-type': contentType.includes('application/json')
        ? 'application/json'
        : 'application/x-www-form-urlencoded',
      accept: 'application/json',
    },
    body: raw,
  });
  return passThroughResponse(upstream, env);
}

/** GET /v1/models и POST /v1/responses — CORS-прокси к api.openai.com. */
async function handleApiProxy(request, pathname, env) {
  const authorization = pickAuthorization(request, env);
  if (!authorization) {
    return jsonErrorResponse(
      401,
      'missing_authorization',
      'Требуется заголовок Authorization: Bearer <access_token> (либо настроенный секрет PROXY_BYOK_KEY).',
      env,
    );
  }
  const headers = { authorization };
  const contentType = request.headers.get('content-type');
  if (contentType) headers['content-type'] = contentType;
  const accept = request.headers.get('accept');
  if (accept) headers.accept = accept;

  let body;
  if (request.method === 'POST') {
    const raw = await readBodyWithinLimit(request, env);
    if (raw === null) {
      return jsonErrorResponse(
        413,
        'payload_too_large',
        `Тело запроса больше ${resolveBodyLimitBytes(env)} байт.`,
        env,
      );
    }
    body = raw;
  }
  const upstream = await fetch(`${OPENAI_API_BASE}${pathname}`, {
    method: request.method,
    headers,
    body,
  });
  return passThroughResponse(upstream, env);
}

// ---------------------------------------------------------------------------
// Роутер + экспорт для Workers
// ---------------------------------------------------------------------------

/** Shared-лимитер на isolate (создаётся лениво, лимит из env первого запроса). */
let sharedLimiterInstance = null;
function sharedLimiter(env) {
  if (!sharedLimiterInstance) {
    sharedLimiterInstance = createRateLimiter({
      limit: parseIntEnv(env, 'RATE_LIMIT_PER_MIN', DEFAULT_RATE_LIMIT_PER_MIN),
    });
  }
  return sharedLimiterInstance;
}

/**
 * Роутер прокси. Третий аргумент — инъекция лимитера (для тестов);
 * по умолчанию используется isolate-local shared-инстанс.
 *
 * Маршруты: OPTIONS * (preflight) | GET /oauth/callback | POST /oauth/token |
 * GET /v1/models | POST /v1/responses; иначе 404/405.
 * @param {Request} request
 * @param {{ALLOWED_ORIGIN?: string, RATE_LIMIT_PER_MIN?: string|number, OAUTH_DEEP_LINK_BASE?: string, MAX_BODY_BYTES?: string|number, PROXY_BYOK_KEY?: string}} [env]
 * @param {{check(key: string): {allowed: boolean, remaining: number, retryAfterSec: number}}} [limiter]
 */
export async function handleRequest(request, env = {}, limiter = null) {
  let url;
  try {
    url = new URL(request.url);
  } catch {
    return jsonErrorResponse(400, 'bad_request', 'Некорректный URL запроса.', {});
  }
  const method = request.method;

  // CORS preflight — всегда, до рейт-лимита (не расходует квоту).
  if (method === 'OPTIONS') {
    return new Response(null, {
      status: 204,
      headers: { ...corsHeaders(env), ...securityHeaders(), 'access-control-max-age': CORS_MAX_AGE_SEC },
    });
  }

  // Рейт-лимит (best-effort, per-isolate).
  const verdict = (limiter ?? sharedLimiter(env)).check(clientIp(request));
  if (!verdict.allowed) {
    return jsonErrorResponse(
      429,
      'rate_limited',
      `Слишком много запросов. Повторите примерно через ${verdict.retryAfterSec} c.`,
      env,
      { 'retry-after': String(verdict.retryAfterSec) },
    );
  }

  try {
    if (url.pathname === CALLBACK_PATH) {
      if (method !== 'GET') {
        return jsonErrorResponse(405, 'method_not_allowed', 'Метод не поддерживается. Только GET.', env, {
          allow: 'GET, OPTIONS',
        });
      }
      return handleCallback(url, env);
    }
    if (url.pathname === TOKEN_PATH) {
      if (method !== 'POST') {
        return jsonErrorResponse(405, 'method_not_allowed', 'Метод не поддерживается. Только POST.', env, {
          allow: 'POST, OPTIONS',
        });
      }
      return await handleTokenExchange(request, env);
    }
    if (url.pathname === RESPONSES_PATH || url.pathname === MODELS_PATH) {
      const expected = url.pathname === MODELS_PATH ? 'GET' : 'POST';
      if (method !== expected) {
        return jsonErrorResponse(
          405,
          'method_not_allowed',
          `Метод не поддерживается. Только ${expected}.`,
          env,
          { allow: `${expected}, OPTIONS` },
        );
      }
      return await handleApiProxy(request, url.pathname, env);
    }
    return jsonErrorResponse(404, 'not_found', 'Неизвестный путь.', env);
  } catch (err) {
    // Безопасное логирование: URL с redact, без тел (в них code/access_token).
    const reason = err && err.message ? err.message : 'unknown';
    console.error(`[llm-proxy] ошибка обработки ${method} ${redactQueryForLog(request.url)}: ${reason}`);
    return jsonErrorResponse(502, 'proxy_error', 'Внутренняя ошибка прокси.', env);
  }
}

/** Точка входа Cloudflare Workers. */
export default {
  async fetch(request, env) {
    return handleRequest(request, env);
  },
};
