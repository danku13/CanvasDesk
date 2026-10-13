// W5-сценарий (#75): web-стек после апгрейда wgpu 30 — WebGPU и WebGL2-фолбэк.
//
// Отличия от lay81-сценария: два режима GPU (MODE=webgpu|webgl2), оракулы
// чек-листа брифа W5:
//   1. адаптер получен — console-логи `GpuContext::new: адаптер получен`
//      и `web GPU-бэкенд выбран`;
//   2. retry не зациклился — ровно ≤3 попытки `request_adapter` (в webgl2-
//      режиме с выключенным WebGPU: 3 null-попытки → фолбэк на GL);
//   3. канва рисует — canvas есть + пиксельный дифф A→B (стабильность)
//      и B→C (ховер) по рецепту LAY8.1;
//   4. кириллица шейпится — текст в кадре: variance-оракул (доля тёмных
//      пикселей текста на светлой панели демо, грубая проверка что глифы
//      вообще есть; тонкая проверка — глаз владельца на pages-preview);
//   5. инлайн-редактирование — dblclick + ввод кириллицы + пиксельный
//      дифф кадра;
//   6. скролл/зум — wheel-события + пиксельный дифф кадра.
//
// Запуск (стенд уже собран — wasm_ui_test.sh --no-build):
//   MODE=webgpu  node scripts/wasm_ui_w5_scenario.mjs   # WebGPU (SwiftShader-Vulkan)
//   MODE=webgl2  node scripts/wasm_ui_w5_scenario.mjs   # WebGL2-фолбэк (WebGPU выключен)
import { createRequire } from 'module';
import { spawn, execSync } from 'child_process';
import { existsSync, mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');

const MODE = process.env.MODE ?? 'webgpu';
const DIST = process.env.WASM_UI_DIST ?? path.join(REPO, 'target/dist');
const OUT = process.env.WASM_UI_OUT ?? path.join(REPO, 'target/wasm_ui');
const PORT = process.env.WASM_UI_PORT ?? '8081';
const LABEL = process.env.LABEL ?? `w5_${MODE}`;
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html?ui=demo`;
const WATCHDOG_MS = Number(process.env.WASM_UI_WATCHDOG_S ?? 300) * 1000;
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

const CHROME_CANDIDATES = [
  process.env.WASM_UI_CHROME,
  '/home/z/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome',
  '/home/z/.cache/ms-playwright/chromium-1248/chrome-linux/chrome',
  '/home/z/.cache/ms-playwright/chromium-1200/chrome-linux/chrome',
  '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome',
].filter(Boolean);
const CHROME = CHROME_CANDIDATES.find(existsSync);

// Режим webgpu: флаги из LAY8.1-рецепта (Dawn → SwiftShader-Vulkan).
// Режим webgl2: БЕЗ WebGPU-флагов + явное выключение фичи — requestAdapter
// даёт null, приложение обязано упасть на ступень 2 (GL/WebGL2).
const baseArgs = ['--no-sandbox', '--disable-gpu-sandbox', `--window-size=${VW},${VH}`, '--window-position=0,0'];
const args = MODE === 'webgpu'
  ? [...baseArgs, '--enable-unsafe-webgpu', '--enable-features=Vulkan',
     '--use-vulkan=swiftshader', '--use-webgpu-adapter=swiftshader']
  : [...baseArgs, '--disable-features=WebGPU,Vulkan'];

const browser = await chromium.launch({
  headless: false, // живой композитор под Xvfb: headless не композитит WebGPU-канвас
  ...(CHROME ? { executablePath: CHROME } : {}),
  args,
});

const logs = [];
const verdict = {};
try {
  const page = await browser.newPage({ viewport: { width: VW, height: VH } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));

  const shot = (name) => page.screenshot({ path: `${OUT}/${LABEL}_${name}.png` });
  const diff = (a, b) => {
    try {
      return Number(execSync(
        `python3 ${path.join(REPO, 'scripts/wasm_ui_diff.py')} ${a} ${b}`,
        { encoding: 'utf8' }).trim());
    } catch (e) { return NaN; }
  };
  const f = (n) => `${OUT}/${LABEL}_${n}.png`;

  console.log(`[шаг] загрузка ${URL} (режим ${MODE}, вьюпорт ${VW}x${VH})…`);
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(12000); // dev-wasm + первый кадр + прогрев
  await shot('a_settled');

  // Оракул 1+2: адаптер и retry — по console-логам (tracing → console.info/warn)
  const adapterOk = logs.some((l) => l.includes('GpuContext::new: адаптер получен'));
  const backend = MODE === 'webgpu' ? 'BROWSER_WEBGPU' : 'GL';
  const backendOk = logs.some((l) => l.includes(backend));
  const attempts = logs.filter((l) => l.includes('request_adapter')).length;
  const fallbackTrail = logs.some((l) => l.includes('фолбэк на GL'));
  verdict.adapter_ok = adapterOk;
  verdict.backend_line_seen = backendOk;
  verdict.adapter_attempts = attempts;
  verdict.fallback_trail_seen = fallbackTrail;
  console.log(`[оракул] адаптер получен: ${adapterOk}; строка бэкенда ${backend}: ${backendOk}; попыток request_adapter: ${attempts}`);

  // Оракул 3: канва рисает — кадр не пустой (доля нетематических пикселей)
  const variance = Number(execSync(
    `python3 -c "import numpy as np; from PIL import Image; a=np.asarray(Image.open('${f('a_settled')}').convert('L'),dtype=np.int16); print(round(float((a<80).mean())*100,2))"`,
    { encoding: 'utf8' }).trim());
  verdict.dark_pixel_pct = variance;
  console.log(`[оракул] доля тёмных пикселей кадра (текст/панели): ${variance}%`);

  await page.waitForTimeout(2000);
  await shot('b_stable');
  verdict.stab_px = diff(f('a_settled'), f('b_stable'));
  console.log(`[оракул] A→B стабильность кадра: ${verdict.stab_px} px`);

  // Оракул 6: скролл/зум — wheel по канве
  await page.mouse.move(VW / 2, VH / 2);
  await page.mouse.wheel(0, -480); // zoom-in (жест приложения)
  await page.waitForTimeout(1200);
  await shot('c_wheel');
  verdict.wheel_px = diff(f('b_stable'), f('c_wheel'));
  await page.mouse.wheel(0, 480); // вернуть
  await page.waitForTimeout(800);

  // Оракул 5: инлайн-редактирование — dblclick по центру + кириллица
  await page.mouse.dblclick(VW / 2, VH / 2);
  await page.waitForTimeout(700);
  await page.keyboard.type('Правка W5 кириллица', { delay: 40 });
  await page.waitForTimeout(1200);
  await shot('d_edit');
  verdict.edit_px = diff(f('c_wheel'), f('d_edit'));
  await page.keyboard.press('Escape');
  console.log(`[оракул] wheel-дифф: ${verdict.wheel_px} px; edit-дифф: ${verdict.edit_px} px`);

  // Панics/ошибки — их не должно быть
  const pageErrors = logs.filter((l) => l.startsWith('[pageerror]') || l.includes('panicked'));
  verdict.page_errors = pageErrors.length;
  console.log(`[оракул] page-error/panic: ${pageErrors.length}`);

  console.log('--- ВЕРДИКТ W5 ---');
  console.log(JSON.stringify(verdict, null, 2));
} finally {
  const { writeFileSync } = await import('fs');
  writeFileSync(`${OUT}/${LABEL}_logs.txt`, logs.join('\n'), 'utf-8');
  console.log('--- LOGS (релевантные) ---');
  for (const l of logs.filter((l) =>
    /request_adapter|GPU|бэкенд|адаптер|wgpu|Atlas|glyph|shape|panic|error/i.test(l)).slice(0, 40)) console.log(l);
  await browser.close().catch(() => {});
  stop();
}
console.log('SCENARIO_DONE');
