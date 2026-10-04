// CR-018 волна v2 — приёмочный сценарий wasm-стенда (L2), v2.
// Путь 1: шаблон из палитры (готовая numi-таблица) — скриншоты + hover ряда.
// Путь 2: однострочная нода с ошибкой (без Enter) — инлайн-ошибка.
import { createRequire } from 'module';
import { spawn } from 'child_process';
import { existsSync, mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');

const DIST = process.env.WASM_UI_DIST ?? path.join(REPO, 'target/dist');
const OUT = process.env.WASM_UI_OUT ?? path.join(REPO, 'target/wasm_ui_cr018');
const PORT = process.env.WASM_UI_PORT ?? '8087';
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html`;
const WATCHDOG_MS = Number(process.env.WASM_UI_WATCHDOG_S ?? 300) * 1000;
// Hover: точка строки шаблонной ноды (калибруется по 03_template.png)
const HOVER = (process.env.HOVER ?? '0,0').split(',').map(Number);

mkdirSync(OUT, { recursive: true });

const server = spawn('python3', ['-m', 'http.server', PORT, '--bind', '127.0.0.1'], {
  cwd: DIST, stdio: 'ignore',
});
const stop = () => { try { server.kill(); } catch { /* уже мёртв */ } };
process.on('exit', stop);
const watchdog = setTimeout(() => {
  console.error('[сценарий] watchdog — принудительный выход');
  process.exit(3);
}, WATCHDOG_MS);
watchdog.unref();

const CHROME = process.env.WASM_UI_CHROME
  ?? '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
const browser = await chromium.launch({
  headless: false,
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
  const shot = (name) => page.screenshot({ path: `${OUT}/${name}.png` });

  console.log('[шаг] загрузка стенда…');
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(7000);

  // Первый запуск: «Продолжить без роли», затем «Пропустить» онбординга
  await page.mouse.click(633, 497);
  await page.waitForTimeout(900);
  await page.mouse.click(812, 299);
  await page.waitForTimeout(700);
  await shot('01_after_skip');

  // Палитра шаблонов: категория «Юнит-экономика · 24»
  console.log('[шаг] палитра шаблонов…');
  await page.mouse.click(100, 427);
  await page.waitForTimeout(900);
  await shot('02_palette');

  // Клик по строке шаблона (первая строка списка — координаты по 02)
  await page.mouse.click(100, 380);
  await page.waitForTimeout(1200);
  await shot('03_template');

  // Hover над строкой данных шаблонной ноды (если HOVER задан)
  if (HOVER[0] > 0) {
    await page.mouse.move(HOVER[0], HOVER[1]);
    await page.waitForTimeout(700);
    await shot('04_hover_row');
    await page.mouse.move(30, 770);
    await page.waitForTimeout(500);
  }

  // Путь 2: однострочная нода с ошибкой — двойной клик по пустому месту,
  // заголовок пропускаем (Enter), в теле одна строка без Enter, клик мимо.
  console.log('[шаг] нода с ошибкой…');
  await page.mouse.click(430, 620, { clickCount: 2 });
  await page.waitForTimeout(900);
  await page.keyboard.type('итог', { delay: 25 });
  await page.keyboard.press('Enter'); // коммит заголовка → тело (цепочка)
  await page.waitForTimeout(500);
  await page.keyboard.type('брак = несуществующее + 1', { delay: 18 });
  await page.waitForTimeout(600);
  await page.mouse.click(30, 770); // клик мимо — коммит тела
  await page.waitForTimeout(1200);
  await shot('05_error_node');

  console.log('[шаг] скриншоты сняты');
} finally {
  clearTimeout(watchdog);
  await browser.close().catch(() => {});
  stop();
  for (const l of logs) console.log(l);
}
