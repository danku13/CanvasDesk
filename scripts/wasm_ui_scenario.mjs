// Переиспользуемый UI-сценарий wasm-стенда (docs/WASM-TESTING.md, уровень L2).
// Родился из проверки фикса 52027bc («панели закрываются при клике на себя»),
// обобщён в env-параметрах: онбординг → меню «?» → пункт → клик по ТЕЛУ
// панели → клик по фону (backdrop). Для другой проверки — свои координаты
// или другой сценарий через SCENARIO=... (контракт: сервер стенда сценарий
// поднимает сам, фоновые процессы между bash-вызовами не выживают).
//
// Оракулы: консоль браузера (backend=BrowserWebGpu, pageerror), скриншоты
// + пиксельные диффы (scripts/wasm_ui_diff.py — считает число изменённых
// пикселей), DOM (body > canvas).
//
// Параметры (env; координаты — CSS-px вьюпорта 1280x800, калибруются по
// скриншотам шага; "0" отключает шаг):
//   WASM_UI_DIST   каталог стенда      (по умолчанию <репо>/target/dist)
//   WASM_UI_OUT    каталог скриншотов  (по умолчанию <репо>/target/wasm_ui)
//   WASM_UI_PORT   порт http.server    (по умолчанию 8081)
//   LABEL          префикс скриншотов  (по умолчанию "ui")
//   SKIP="x,y"     «Пропустить» онбординга     (по умолчанию 812,299)
//   HELP="x,y"     кнопка «?» — нижняя часть   (по умолчанию 1158,40; DOM-тулбар перекрывает верх)
//   ITEM="x,y"     пункт меню (панель/фича, которую проверяем)   — обязателен
//   BODY="x,y"     точка ТЕЛА панели (паддинг вне интерактивных) — обязательна
//   BACKDROP="x,y" точка фона вне панели       (по умолчанию 30,770)
//   WASM_UI_WATCHDOG_S  таймаут зависания (по умолчанию 240)
//
// Запуск (обычно через scripts/wasm_ui_test.sh — он поднимает Xvfb сам,
// xvfb-run в среде ломается из-за отсутствия xauth):
//   scripts/wasm_ui_test.sh --no-build
import { createRequire } from 'module';
import { spawn, execSync } from 'child_process';
import { existsSync, mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');

const DIST = process.env.WASM_UI_DIST ?? path.join(REPO, 'target/dist');
const OUT = process.env.WASM_UI_OUT ?? path.join(REPO, 'target/wasm_ui');
const PORT = process.env.WASM_UI_PORT ?? '8081';
const LABEL = process.env.LABEL ?? 'ui';
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html`;
const WATCHDOG_MS = Number(process.env.WASM_UI_WATCHDOG_S ?? 240) * 1000;

const xy = (v, fallback) => {
  if (v === undefined) return fallback;
  if (v === '0') return null; // шаг отключён
  return v.split(',').map(Number);
};
const SKIP = xy(process.env.SKIP, [812, 299]);
const HELP = xy(process.env.HELP, [1158, 40]);
const ITEM = xy(process.env.ITEM, undefined);
const BODY = xy(process.env.BODY, undefined);
const BACKDROP = xy(process.env.BACKDROP, [30, 770]);
if (!ITEM || !BODY) {
  console.error('нужны ITEM="x,y" и BODY="x,y" (что открыть и где тело панели)');
  process.exit(2);
}

mkdirSync(OUT, { recursive: true });

// Сервер стенда — ребёнок ЭТОГО процесса: фоновые процессы между bash-вызовами
// не выживают. Гасим явно в finally (процесс-ребёнок держит event loop —
// без явного kill node не завершится сам; найдено первым прогоном рецепта).
const server = spawn('python3', ['-m', 'http.server', PORT, '--bind', '127.0.0.1'], {
  cwd: DIST, stdio: 'ignore',
});
const stop = () => {
  try { server.kill(); } catch { /* уже мёртв */ }
};
process.on('exit', stop);

// Watchdog: сценарий завис (стенд не поднялся, браузер не стартовал) —
// уходим по таймауту, finally погасит сервер.
const watchdog = setTimeout(() => {
  console.error(`[сценарий] watchdog ${WATCHDOG_MS / 1000} с — принудительный выход`);
  process.exit(3);
}, WATCHDOG_MS);
watchdog.unref(); // не держим loop при штатном завершении

const CHROME = process.env.WASM_UI_CHROME
  ?? '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
const browser = await chromium.launch({
  headless: false, // живой композитор под Xvfb: headless не видит WebGPU-канвас
  ...(existsSync(CHROME) ? { executablePath: CHROME } : {}),
  args: [
    '--no-sandbox', '--disable-gpu-sandbox',
    '--enable-unsafe-webgpu', '--enable-features=Vulkan',
    '--use-vulkan=swiftshader', '--use-webgpu-adapter=swiftshader',
    '--window-size=1280,800', '--window-position=0,0',
  ],
});

const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));

  const shot = (name) => page.screenshot({ path: `${OUT}/${LABEL}_${name}.png` });
  const click = async (x, y, ms = 700) => {
    await page.mouse.move(x, y);
    await page.waitForTimeout(150);
    await page.mouse.click(x, y);
    await page.waitForTimeout(ms);
  };

  // Долгая загрузка dev-wasm (~34 МБ) + первый кадр
  console.log('[шаг] загрузка стенда…');
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(7000);
  await shot('00_initial');
  console.log('[шаг] стенд загружен, канвас в DOM');

  // Онбординг: «Пропустить» (чип в шапке карточки)
  if (SKIP) {
    await click(SKIP[0], SKIP[1]);
    await shot('01_idle');
    console.log('[шаг] онбординг пропущен');
  }

  // Меню «?» → пункт → панель открыта
  await click(HELP[0], HELP[1]);
  await shot('02_menu');
  console.log('[шаг] меню «?» открыто');
  await click(ITEM[0], ITEM[1]);

  // Курсор в точку тела ДО контрольного кадра — гасим hover-артефакты в диффе
  await page.mouse.move(BODY[0], BODY[1]);
  await page.waitForTimeout(400);
  await shot('03_open_control');
  console.log('[шаг] панель открыта, контрольный кадр снят');
  await click(BODY[0], BODY[1]);
  await shot('04_after_body_click');
  console.log('[шаг] клик по телу панели выполнен');

  // Клик по фону снаружи — backdrop-контракт: панель закрывается
  if (BACKDROP) {
    await click(BACKDROP[0], BACKDROP[1]);
    await shot('05_after_backdrop');
    console.log('[шаг] клик по фону выполнен');
  }

  // Пиксельный дифф: (03 → 04) панель осталась, (03/04 → 05) панель закрылась
  const diff = (a, b) => {
    const cmd = `python3 ${path.join(REPO, 'scripts/wasm_ui_diff.py')} ${a} ${b}`;
    try {
      return execSync(cmd, { encoding: 'utf8' }).trim();
    } catch (e) {
      return `n/a (${e.message.split('\n')[0]})`;
    }
  };
  const f = (n) => `${OUT}/${LABEL}_${n}.png`;
  console.log('--- ПИКСЕЛЬНЫЕ ДИФФЫ ---');
  console.log(`открыто→клик по телу (03→04): ${diff(f('03_open_control'), f('04_after_body_click'))} px`);
  if (BACKDROP) {
    console.log(`клик по телу→фон (04→05): ${diff(f('04_after_body_click'), f('05_after_backdrop'))} px`);
  }
} finally {
  console.log('--- LOGS (последние 10) ---');
  for (const l of logs.slice(-10)) console.log(l);
  await browser.close().catch(() => {});
  stop();
}
console.log('SCENARIO_DONE');
