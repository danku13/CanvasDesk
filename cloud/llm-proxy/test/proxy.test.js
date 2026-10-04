/**
 * Тесты canvasdesk-llm-proxy (FR-LLM-PROXY) — чистые функции worker.js +
 * роутер handleRequest без wrangler (node --test test/).
 *
 * Сеть не используется: upstream fetch подменяется моком только там, где
 * проверяется проброс (token exchange, /v1/*); все прочие маршруты завершаются
 * до обращения к upstream.
 *
 * Покрывает: F-5.10 (callback redirect), Q6 (stateless pass-through),
 * CORS, рейт-лимитер с инъекцией часов, лимит тела 1 МБ, 404/405/413/429.
 */
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';

import {
  buildCallbackRedirect,
  clientIp,
  corsHeaders,
  createRateLimiter,
  handleRequest,
  parseTokenBody,
  pickAuthorization,
  redactQueryForLog,
  securityHeaders,
  validateTokenBody,
} from '../worker.js';

// ---------------------------------------------------------------------------
// Хелперы
// ---------------------------------------------------------------------------

/** Прокси-base для тестовых URL. */
const PROXY = 'https://llm-proxy.canvasdesk.test';

/** Подменяет globalThis.fetch на мок, возвращает { restore, calls } . */
function mockFetch(responder) {
  const original = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (url, init) => {
    calls.push({ url: String(url), init: init || {} });
    return responder(calls[calls.length - 1]);
  };
  return {
    calls,
    restore() {
      globalThis.fetch = original;
    },
  };
}

const FORM_EXCHANGE =
  'grant_type=authorization_code&code=abc123&code_verifier=verifier456&client_id=client789' +
  '&redirect_uri=https%3A%2F%2Fapp.canvasdesk.test%2Foauth%2Fbridge&resource=https%3A%2F%2Fapi.openai.com%2Fv1';

// ---------------------------------------------------------------------------
// buildCallbackRedirect — F-5.10
// ---------------------------------------------------------------------------

describe('buildCallbackRedirect (F-5.10)', () => {
  test('валидный запрос → deep-link URL с сохранёнными code/state', () => {
    const target = buildCallbackRedirect(
      `${PROXY}/oauth/callback?code=abc123&state=st-456`,
      'canvasdesk://oauth/callback',
    );
    assert.equal(target, 'canvasdesk://oauth/callback?code=abc123&state=st-456');
  });

  test('прочие query-параметры (scope) сохраняются', () => {
    const target = buildCallbackRedirect(
      `${PROXY}/oauth/callback?code=c1&state=s1&scope=openid%20profile`,
      'canvasdesk://oauth/callback',
    );
    const params = new URLSearchParams(target.split('?')[1]);
    assert.equal(params.get('code'), 'c1');
    assert.equal(params.get('state'), 's1');
    assert.equal(params.get('scope'), 'openid profile');
  });

  test('спецсимволы кода сохраняются (кодирование туда-обратно)', () => {
    const target = buildCallbackRedirect(
      `${PROXY}/oauth/callback?code=a%2Bb%3Dc&state=x`,
      'canvasdesk://oauth/callback',
    );
    const params = new URLSearchParams(target.split('?')[1]);
    assert.equal(params.get('code'), 'a+b=c');
  });

  test('нет code → null', () => {
    assert.equal(buildCallbackRedirect(`${PROXY}/oauth/callback?state=s1`), null);
  });

  test('нет state → null', () => {
    assert.equal(buildCallbackRedirect(`${PROXY}/oauth/callback?code=c1`), null);
  });

  test('нет параметров вовсе → null', () => {
    assert.equal(buildCallbackRedirect(`${PROXY}/oauth/callback`), null);
  });

  test('невалидный URL → null', () => {
    assert.equal(buildCallbackRedirect('not-a-url'), null);
  });
});

// ---------------------------------------------------------------------------
// validateTokenBody / parseTokenBody — pass-through token exchange
// ---------------------------------------------------------------------------

describe('validateTokenBody', () => {
  test('authorization_code с обязательными полями → true', () => {
    assert.equal(
      validateTokenBody({
        grant_type: 'authorization_code',
        code: 'abc',
        code_verifier: 'ver',
        redirect_uri: 'https://app.test/cb',
        client_id: 'cid',
      }),
      true,
    );
  });

  test('authorization_code без code_verifier → false', () => {
    assert.equal(
      validateTokenBody({
        grant_type: 'authorization_code',
        code: 'abc',
        redirect_uri: 'https://app.test/cb',
        client_id: 'cid',
      }),
      false,
    );
  });

  test('refresh_token с refresh_token + client_id → true', () => {
    assert.equal(
      validateTokenBody({ grant_type: 'refresh_token', refresh_token: 'rt', client_id: 'cid' }),
      true,
    );
  });

  test('refresh_token без refresh_token → false', () => {
    assert.equal(validateTokenBody({ grant_type: 'refresh_token', client_id: 'cid' }), false);
  });

  test('чужие гранты (client_credentials/password) → false', () => {
    assert.equal(validateTokenBody({ grant_type: 'client_credentials' }), false);
    assert.equal(validateTokenBody({ grant_type: 'password', username: 'u', password: 'p' }), false);
  });

  test('мусор → false (null, массив, строка, пустой объект, нет grant_type)', () => {
    assert.equal(validateTokenBody(null), false);
    assert.equal(validateTokenBody(['grant_type']), false);
    assert.equal(validateTokenBody('grant_type=authorization_code'), false);
    assert.equal(validateTokenBody({}), false);
    assert.equal(validateTokenBody({ code: 'x' }), false);
  });
});

describe('parseTokenBody', () => {
  test('JSON → объект', () => {
    const body = parseTokenBody(
      '{"grant_type":"refresh_token","refresh_token":"rt","client_id":"cid"}',
      'application/json; charset=utf-8',
    );
    assert.equal(body.grant_type, 'refresh_token');
    assert.equal(body.refresh_token, 'rt');
  });

  test('urlencoded (как шлёт OpenAI) → объект', () => {
    const body = parseTokenBody('grant_type=authorization_code&code=a%2Bb', 'application/x-www-form-urlencoded');
    assert.equal(body.grant_type, 'authorization_code');
    assert.equal(body.code, 'a+b');
  });

  test('битое JSON-тело → null', () => {
    assert.equal(parseTokenBody('{oops', 'application/json'), null);
  });

  test('неизвестный content-type → null', () => {
    assert.equal(parseTokenBody('anything', 'text/plain'), null);
  });
});

// ---------------------------------------------------------------------------
// createRateLimiter — окно 60с, время инъецируется (без setTimeout)
// ---------------------------------------------------------------------------

describe('createRateLimiter', () => {
  test('N запросов внутри окна: лимит срабатывает, retryAfter > 0', () => {
    let t = 1_000;
    const rl = createRateLimiter({ limit: 3, windowMs: 60_000, now: () => t });
    assert.equal(rl.check('ip1').allowed, true);
    assert.equal(rl.check('ip1').allowed, true);
    assert.equal(rl.check('ip1').allowed, true);
    const blocked = rl.check('ip1');
    assert.equal(blocked.allowed, false);
    assert.ok(blocked.retryAfterSec > 0 && blocked.retryAfterSec <= 60);
  });

  test('после окна — снова пускает (счётчик сбрасывается)', () => {
    let t = 1_000;
    const rl = createRateLimiter({ limit: 2, windowMs: 60_000, now: () => t });
    assert.equal(rl.check('ip1').allowed, true);
    assert.equal(rl.check('ip1').allowed, true);
    assert.equal(rl.check('ip1').allowed, false);
    t += 60_001; // окно истекло — время инъецировано, никаких таймеров
    assert.equal(rl.check('ip1').allowed, true);
    assert.equal(rl.check('ip1').allowed, true);
    assert.equal(rl.check('ip1').allowed, false);
  });

  test('ключи (IP) независимы', () => {
    let t = 0;
    const rl = createRateLimiter({ limit: 1, windowMs: 60_000, now: () => t });
    assert.equal(rl.check('ip-a').allowed, true);
    assert.equal(rl.check('ip-a').allowed, false);
    assert.equal(rl.check('ip-b').allowed, true);
  });

  test('remaining уменьшается с каждым запросом', () => {
    let t = 0;
    const rl = createRateLimiter({ limit: 3, windowMs: 60_000, now: () => t });
    assert.equal(rl.check('ip1').remaining, 2);
    assert.equal(rl.check('ip1').remaining, 1);
    assert.equal(rl.check('ip1').remaining, 0);
  });

  test('защита памяти: старые ключи выметаются при переполнении maxEntries', () => {
    let t = 0;
    const rl = createRateLimiter({ limit: 1, windowMs: 1_000, now: () => t, maxEntries: 3 });
    rl.check('a');
    rl.check('b');
    rl.check('c'); // maxEntries достигнут
    t += 2_000; // все записи просрочены
    rl.check('d');
    assert.ok(rl.size() <= 3);
  });
});

// ---------------------------------------------------------------------------
// corsHeaders / securityHeaders / pickAuthorization
// ---------------------------------------------------------------------------

describe('corsHeaders', () => {
  test('по умолчанию origin «*», методы GET/POST/OPTIONS, заголовки Authorization/Content-Type', () => {
    const h = corsHeaders({});
    assert.equal(h['access-control-allow-origin'], '*');
    assert.equal(h['access-control-allow-methods'], 'GET, POST, OPTIONS');
    assert.ok(h['access-control-allow-headers'].includes('Authorization'));
    assert.ok(h['access-control-allow-headers'].includes('Content-Type'));
    assert.ok(Number(h['access-control-max-age']) >= 600);
  });

  test('env.ALLOWED_ORIGIN пробрасывается в Access-Control-Allow-Origin', () => {
    const h = corsHeaders({ ALLOWED_ORIGIN: 'https://app.canvasdesk.dev' });
    assert.equal(h['access-control-allow-origin'], 'https://app.canvasdesk.dev');
  });

  test('securityHeaders: nosniff на каждом ответе', () => {
    assert.equal(securityHeaders()['x-content-type-options'], 'nosniff');
  });
});

describe('pickAuthorization (BYOK-through-proxy)', () => {
  test('Authorization пользователя пробрасывается как есть', () => {
    const req = new Request(`${PROXY}/v1/models`, {
      headers: { authorization: 'Bearer user-oauth-token' },
    });
    assert.equal(pickAuthorization(req, {}), 'Bearer user-oauth-token');
  });

  test('нет заголовка + PROXY_BYOK_KEY → Bearer <ключ из env>', () => {
    const req = new Request(`${PROXY}/v1/models`);
    assert.equal(pickAuthorization(req, { PROXY_BYOK_KEY: 'sk-proxy' }), 'Bearer sk-proxy');
  });

  test('нет заголовка и ключа → null (будет 401)', () => {
    assert.equal(pickAuthorization(new Request(`${PROXY}/v1/models`), {}), null);
  });

  test('clientIp: CF-Connecting-IP → X-Forwarded-For → unknown', () => {
    assert.equal(
      clientIp(new Request(PROXY, { headers: { 'cf-connecting-ip': '203.0.113.7' } })),
      '203.0.113.7',
    );
    assert.equal(clientIp(new Request(PROXY, { headers: { 'x-forwarded-for': '198.51.100.2, 10.0.0.1' } })), '198.51.100.2');
    assert.equal(clientIp(new Request(PROXY)), 'unknown');
  });
});

// ---------------------------------------------------------------------------
// redactQueryForLog — токены/code никогда не попадают в логи
// ---------------------------------------------------------------------------

describe('redactQueryForLog', () => {
  test('code/state/access_token заменяются на <redacted>', () => {
    const safe = redactQueryForLog(`${PROXY}/oauth/callback?code=TOPSECRET&state=S2&x=ok`);
    assert.ok(!safe.includes('TOPSECRET'));
    assert.ok(!safe.includes('S2'));
    assert.ok(safe.includes('x=ok'));
    assert.ok(safe.includes('redacted'));
  });
});

// ---------------------------------------------------------------------------
// handleRequest — роутер (сеть не используется)
// ---------------------------------------------------------------------------

describe('handleRequest: CORS preflight и ошибки маршрутизации', () => {
  test('OPTIONS * → 204 + CORS-заголовки', async () => {
    const res = await handleRequest(new Request(`${PROXY}/v1/responses`, { method: 'OPTIONS' }), {});
    assert.equal(res.status, 204);
    assert.equal(res.headers.get('access-control-allow-origin'), '*');
    assert.equal(res.headers.get('access-control-allow-methods'), 'GET, POST, OPTIONS');
    assert.equal(res.headers.get('x-content-type-options'), 'nosniff');
  });

  test('неизвестный путь → 404', async () => {
    const res = await handleRequest(new Request(`${PROXY}/nope`), {});
    assert.equal(res.status, 404);
    assert.equal(res.headers.get('x-content-type-options'), 'nosniff');
  });

  test('неверный метод на известном пути → 405 + Allow', async () => {
    const resPost = await handleRequest(new Request(`${PROXY}/oauth/callback`, { method: 'POST' }), {});
    assert.equal(resPost.status, 405);
    assert.ok(resPost.headers.get('allow').includes('GET'));

    const resGet = await handleRequest(new Request(`${PROXY}/v1/responses`), {});
    assert.equal(resGet.status, 405);
    assert.ok(resGet.headers.get('allow').includes('POST'));

    const resModels = await handleRequest(new Request(`${PROXY}/v1/models`, { method: 'POST' }), {});
    assert.equal(resModels.status, 405);
    assert.ok(resModels.headers.get('allow').includes('GET'));
  });

  test('GET /oauth/callback без code/state → 400 (значения не эхоятся)', async () => {
    const res = await handleRequest(new Request(`${PROXY}/oauth/callback`), {});
    assert.equal(res.status, 400);
    const text = await res.text();
    assert.ok(!text.includes('TOPSECRET'));
  });
});

describe('handleRequest: GET /oauth/callback (F-5.10)', () => {
  test('валидный → 302 на canvasdesk://oauth/callback с теми же параметрами', async () => {
    const res = await handleRequest(
      new Request(`${PROXY}/oauth/callback?code=abc123&state=st-456`),
      {},
    );
    assert.equal(res.status, 302);
    assert.equal(res.headers.get('location'), 'canvasdesk://oauth/callback?code=abc123&state=st-456');
    assert.equal(res.headers.get('x-content-type-options'), 'nosniff');
    assert.equal(res.headers.get('cache-control'), 'no-store');
  });

  test('deep-link base переопределяется через env.OAUTH_DEEP_LINK_BASE', async () => {
    const res = await handleRequest(
      new Request(`${PROXY}/oauth/callback?code=c&state=s`),
      { OAUTH_DEEP_LINK_BASE: 'canvasdesk-dev://oauth/callback' },
    );
    assert.equal(res.status, 302);
    assert.ok(res.headers.get('location').startsWith('canvasdesk-dev://oauth/callback?'));
  });
});

describe('handleRequest: POST /oauth/token (stateless pass-through, Q6)', () => {
  test('валидный form-тело уходит как есть на auth.openai.com, ответ — как есть', async () => {
    const m = mockFetch(
      () =>
        new Response('{"access_token":"AT","refresh_token":"RT"}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
    );
    try {
      const req = new Request(`${PROXY}/oauth/token`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: FORM_EXCHANGE,
      });
      const res = await handleRequest(req, {});
      assert.equal(m.calls.length, 1);
      assert.equal(m.calls[0].url, 'https://auth.openai.com/oauth/token');
      assert.equal(m.calls[0].init.method, 'POST');
      assert.equal(m.calls[0].init.body, FORM_EXCHANGE); // как есть, без перезаписи
      assert.equal(m.calls[0].init.headers['content-type'], 'application/x-www-form-urlencoded');

      assert.equal(res.status, 200);
      assert.equal(await res.text(), '{"access_token":"AT","refresh_token":"RT"}'); // ответ как есть
      assert.equal(res.headers.get('content-type'), 'application/json');
      assert.equal(res.headers.get('access-control-allow-origin'), '*'); // CORS обязателен (wasm)
      assert.equal(res.headers.get('x-content-type-options'), 'nosniff');
    } finally {
      m.restore();
    }
  });

  test('невалидное тело (не тот grant) → 400, upstream не вызывается', async () => {
    const m = mockFetch(() => new Response('{}', { status: 200 }));
    try {
      const req = new Request(`${PROXY}/oauth/token`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: 'grant_type=client_credentials&client_id=x',
      });
      const res = await handleRequest(req, {});
      assert.equal(res.status, 400);
      assert.equal(m.calls.length, 0);
    } finally {
      m.restore();
    }
  });

  test('битый JSON → 400, upstream не вызывается', async () => {
    const m = mockFetch(() => new Response('{}', { status: 200 }));
    try {
      const req = new Request(`${PROXY}/oauth/token`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: '{oops',
      });
      const res = await handleRequest(req, {});
      assert.equal(res.status, 400);
      assert.equal(m.calls.length, 0);
    } finally {
      m.restore();
    }
  });

  test('тело больше лимита → 413, upstream не вызывается', async () => {
    const m = mockFetch(() => new Response('{}', { status: 200 }));
    try {
      const req = new Request(`${PROXY}/oauth/token`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: `grant_type=refresh_token&refresh_token=${'x'.repeat(100)}&client_id=c`,
      });
      const res = await handleRequest(req, { MAX_BODY_BYTES: '16' });
      assert.equal(res.status, 413);
      assert.equal(m.calls.length, 0);
    } finally {
      m.restore();
    }
  });
});

describe('handleRequest: CORS-прокси /v1/models и /v1/responses', () => {
  test('POST /v1/responses: Authorization как есть, Content-Type и тело пробрасываются', async () => {
    const m = mockFetch(
      () =>
        new Response('{"id":"resp_1"}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
    );
    try {
      const req = new Request(`${PROXY}/v1/responses`, {
        method: 'POST',
        headers: {
          authorization: 'Bearer user-oauth-token',
          'content-type': 'application/json',
        },
        body: '{"model":"gpt-5.1","input":"hi"}',
      });
      const res = await handleRequest(req, {});
      assert.equal(m.calls[0].url, 'https://api.openai.com/v1/responses');
      assert.equal(m.calls[0].init.headers.authorization, 'Bearer user-oauth-token');
      assert.equal(m.calls[0].init.headers['content-type'], 'application/json');
      assert.equal(m.calls[0].init.body, '{"model":"gpt-5.1","input":"hi"}');
      assert.equal(res.status, 200);
      assert.equal(await res.text(), '{"id":"resp_1"}');
      assert.equal(res.headers.get('access-control-allow-origin'), '*');
    } finally {
      m.restore();
    }
  });

  test('GET /v1/models без Authorization, но с PROXY_BYOK_KEY → Bearer ключа из env', async () => {
    const m = mockFetch(
      () => new Response('{"data":[]}', { status: 200, headers: { 'content-type': 'application/json' } }),
    );
    try {
      const req = new Request(`${PROXY}/v1/models`);
      const res = await handleRequest(req, { PROXY_BYOK_KEY: 'sk-proxy-key' });
      assert.equal(m.calls[0].url, 'https://api.openai.com/v1/models');
      assert.equal(m.calls[0].init.headers.authorization, 'Bearer sk-proxy-key');
      assert.equal(res.status, 200);
    } finally {
      m.restore();
    }
  });

  test('GET /v1/models без Authorization и без ключа → 401', async () => {
    const m = mockFetch(() => new Response('{}', { status: 200 }));
    try {
      const res = await handleRequest(new Request(`${PROXY}/v1/models`), {});
      assert.equal(res.status, 401);
      assert.equal(m.calls.length, 0);
    } finally {
      m.restore();
    }
  });

  test('тело /v1/responses больше лимита → 413, upstream не вызывается', async () => {
    const m = mockFetch(() => new Response('{}', { status: 200 }));
    try {
      const req = new Request(`${PROXY}/v1/responses`, {
        method: 'POST',
        headers: { authorization: 'Bearer t', 'content-type': 'application/json' },
        body: JSON.stringify({ input: 'x'.repeat(64) }),
      });
      const res = await handleRequest(req, { MAX_BODY_BYTES: '16' });
      assert.equal(res.status, 413);
      assert.equal(m.calls.length, 0);
    } finally {
      m.restore();
    }
  });
});

describe('handleRequest: рейт-лимит (инъекция лимитера)', () => {
  test('второй запрос в окне → 429 + Retry-After; OPTIONS не расходует квоту', async () => {
    let t = 500;
    const limiter = createRateLimiter({ limit: 1, windowMs: 60_000, now: () => t });

    const first = await handleRequest(
      new Request(`${PROXY}/oauth/callback?code=c&state=s`),
      {},
      limiter,
    );
    assert.equal(first.status, 302);

    const second = await handleRequest(
      new Request(`${PROXY}/oauth/callback?code=c&state=s`),
      {},
      limiter,
    );
    assert.equal(second.status, 429);
    assert.ok(Number(second.headers.get('retry-after')) > 0);
    assert.equal(second.headers.get('access-control-allow-origin'), '*');

    // Preflight не лимитируется и не «съедает» окно.
    t += 60_001;
    const preflight = await handleRequest(new Request(`${PROXY}/v1/models`, { method: 'OPTIONS' }), {}, limiter);
    assert.equal(preflight.status, 204);
    const third = await handleRequest(
      new Request(`${PROXY}/oauth/callback?code=c&state=s`),
      {},
      limiter,
    );
    assert.equal(third.status, 302);
  });
});
