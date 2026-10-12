// L2-сценарий V3 (issue #68): канонические вьюпорты LAY8.1 (?ui=demo —
// витрина кита) + pixel-diff оракул стабильности кадра.
//
// Отличия от дефолтного wasm_ui_scenario.mjs: стенд грузится сразу в
// ?ui=demo (онбординг/меню не участвуют), снимаются два вьюпорта
// (WASM_UI_VIEWPORT, по умолчанию 1280x800; второй прогон — 1024x640),
// оракул — дифф двух кадров с интервалом (стабильность: нет вечной
// перерисовки) и дифф между вьюпортами (рендер живой).
//
// Запуск: scripts/wasm_ui_test.sh --no-build  (сборку делает гейт заранее)
//   SCENARIO=scripts/wasm_ui_lay81_scenario.mjs
//   WASM_UI_URL='http://127.0.0.1:8081/index.html?ui=demo'
//   WASM_UI_VIEWPORT=1280x800 LABEL=lay81_1280
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
const LABEL = process.env.LABEL ?? 'lay81';
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html?ui=demo`;
const WATCHDOG_MS = Number(process.env.WASM_UI_WATCHDOG_S ?? 240) * 1000;
const [VW, VH] = (process.env.WASM_UI_VIEWPORT ?? '1280x800').split('x').map(Number);

mkdirSync(OUT, { recursive: true });

const server = spawn('python3', ['-m', 'http.server', PORT, '--bind', '127.0.0.1'], {
  cwd: DIST, stdio: 'ignore',
});
const stop = () => { try { server.kill(); } catch { /* уже мёртв */ } };
process.on('exit', stop);
const watchdog = setTimeout(() => {
  console.error(`[сценарий] watchdog ${WATCHDOG_MS / 1000} с — принудительный выход`);
  process.exit(3);
}, WATCHDOG_MS);
watchdog.unref();

const CHROME = process.env.WASM_UI_CHROME
  ?? '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
const browser = await chromium.launch({
  headless: false, // живой композитор под Xvfb: headless не видит WebGPU-канвас
  ...(existsSync(CHROME) ? { executablePath: CHROME } : {}),
  args: [
    '--no-sandbox', '--disable-gpu-sandbox',
    '--enable-unsafe-webgpu', '--enable-features=Vulkan',
    '--use-vulkan=swiftshader', '--use-webgpu-adapter=swiftshader',
    `--window-size=${VW},${VH}`, '--window-position=0,0',
  ],
});

const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: VW, height: VH } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));

  const shot = (name) => page.screenshot({ path: `${OUT}/${LABEL}_${name}.png` });
  const diff = (a, b) => {
    const cmd = `python3 ${path.join(REPO, 'scripts/wasm_ui_diff.py')} ${a} ${b}`;
    try {
      return execSync(cmd, { encoding: 'utf8' }).trim();
    } catch (e) {
      return `n/a (${e.message.split('\n')[0]})`;
    }
  };
  const f = (n) => `${OUT}/${LABEL}_${n}.png`;

  console.log(`[шаг] загрузка ${URL} во вьюпорте ${VW}x${VH}…`);
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(9000); // dev-wasm + первый кадр + прогрев

  // Онбординг-проход (экран языка/роли → карточка тура): координаты env,
  // "0" отключает шаг. По умолчанию: «Продолжить без роли» + «Пропустить».
  const click = async (x, y, ms = 900) => {
    await page.mouse.move(x, y);
    await page.waitForTimeout(150);
    await page.mouse.click(x, y);
    await page.waitForTimeout(ms);
  };
  const step = (v, d) => {
    const s = v === undefined ? d : v;
    if (s === null || s === '0') return null;
    return String(s).split(',').map(Number);
  };
  for (const [name, def] of [['ONB1', '640,497'], ['ONB2', '925,311'], ['ONB3', null]]) {
    const p = step(process.env[name], def);
    if (!p) continue;
    await click(p[0], p[1]);
    await shot(`onb_${name}`);
    console.log(`[шаг] онбординг ${name} (${p[0]},${p[1]})`);
  }

  await page.waitForTimeout(3000); // прогрев демо после онбординга
  await shot('a_settled');
  console.log('[шаг] кадр A снят (прогрев завершён)');

  await page.waitForTimeout(2000);
  await shot('b_stable');
  console.log('[шаг] кадр B снят (интервал 2 с)');

  // Живой рендер: наведение на интерактив (дефолт — «Переключить тему»)
  const hp = step(process.env.HOVER, "944,108"); await page.mouse.move(hp[0], hp[1]);
  await page.waitForTimeout(600);
  await shot('c_hover');

  // Клик-оракул интерактивности: клик в ту же точку → состояние меняется
  const cp = step(process.env.CLICK, null);
  if (cp) {
    await click(cp[0], cp[1], 1200);
    await shot('d_after_click');
  }

  console.log('--- ПИКСЕЛЬНЫЕ ДИФФЫ ---');
  console.log(`A→B (стабильность кадра, ожидание ~0): ${diff(f('a_settled'), f('b_stable'))} px`);
  console.log(`B→C (ховер, рендер жив): ${diff(f('b_stable'), f('c_hover'))} px`);
  if (cp) {
    console.log(`C→D (клик, интерактивность): ${diff(f('c_hover'), f('d_after_click'))} px`);
  }
  console.log(`вьюпорт: ${VW}x${VH}, скриншоты: ${OUT}/${LABEL}_*.png`);
} finally {
  console.log('--- LOGS (последние 10) ---');
  for (const l of logs.slice(-10)) console.log(l);
  await browser.close().catch(() => {});
  stop();
}
console.log('SCENARIO_DONE');
