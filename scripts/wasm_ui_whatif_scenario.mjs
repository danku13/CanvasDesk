// L2-сценарий what-if бара (docs/WASM-TESTING.md, уровень L2):
// полный цикл жизни сценариев — вход в режим, создание сценария «+»,
// полный «Сброс» (confirm → Enter), повторное создание, удаление одного
// сценария «✕» чипа (confirm → Enter). Оракулы: скриншоты + пиксельные
// диффы (scripts/wasm_ui_diff.py), консоль браузера (backend, pageerror).
//
// Координаты — вычислены bar_layout для вьюпорта 1280×800, RU-локаль
// (временный тест tmp_print_bar_coords_empty, сессия 2026-10-07):
//   PILL=(640,773) EMPTY_PLUS=(465,757) EMPTY_RESET=(694,757)
//   ONE_PLUS=(527,757) ONE_SCEN_CLOSE=(495,757) ONE_RESET=(756,757)
//
// Запуск: scripts/wasm_ui_test.sh SCENARIO=scripts/wasm_ui_whatif_scenario.mjs
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
const URL = process.env.WASM_UI_URL ?? `http://127.0.0.1:${PORT}/index.html`;
const WATCHDOG_MS = Number(process.env.WASM_UI_WATCHDOG_S ?? 240) * 1000;

// Координаты (см. шапку).
const PILL = [640, 773];
const EMPTY_PLUS = [465, 757];
const EMPTY_RESET = [694, 757];
const ONE_PLUS = [527, 757];
const ONE_SCEN_CLOSE = [495, 757];
const ONE_RESET = [756, 757];

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

  const shot = (name) => page.screenshot({ path: `${OUT}/whatif_${name}.png` });
  const click = async (x, y, ms = 700) => {
    await page.mouse.move(x, y);
    await page.waitForTimeout(150);
    await page.mouse.click(x, y);
    await page.waitForTimeout(ms);
  };
  const enter = async (ms = 500) => {
    await page.keyboard.press('Enter');
    await page.waitForTimeout(ms);
  };
  const hoverAway = async () => {
    await page.mouse.move(30, 770);
    await page.waitForTimeout(400);
  };

  console.log('[шаг] загрузка стенда…');
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(4000);

  // Первичная настройка web-шелла (DOM-оверлей #lang-picker-overlay,
  // FR-087 v2): «Продолжить без роли» → пишет canvasdesk.config в
  // localStorage и ПЕРЕГРУЖАЕТ страницу. До выбора блокирует приложение.
  const langOverlay = await page.$('#lang-picker-overlay');
  if (langOverlay && (await langOverlay.isVisible())) {
    await shot('00a_lang_picker');
    await page.click('#role-skip');
    console.log('[шаг] выбор языка/роли: продолжить без роли (перезагрузка)');
    await page.waitForLoadState('load', { timeout: 60000 });
    await page.waitForSelector('body > canvas', { timeout: 90000 });
    await page.waitForTimeout(7000);
  }
  await shot('00_initial');

  // Онбординг-тур закрываем клавишей Esc (контракт карусели: «Пропустить» —
  // Esc — то же; координата кнопки Skip плывёт с высотой текста шага).
  // После перезагрузки (lang-picker) тур показывается снова — жмём Esc.
  await page.keyboard.press('Escape');
  await page.waitForTimeout(800);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(800);
  await shot('01_idle');
  console.log('[шаг] онбординг-тур пропущен (Esc ×2)');

  // 1) Вход в what-if режим (пилюля bottom-center).
  await click(PILL[0], PILL[1]);
  await hoverAway();
  await shot('02_whatif_enter');
  console.log('[шаг] what-if режим открыт (пилюля)');

  // 2) Создание сценария «+» (пустой бар → EMPTY_PLUS).
  await click(EMPTY_PLUS[0], EMPTY_PLUS[1]);
  await hoverAway();
  await shot('03_scenario_created');
  console.log('[шаг] сценарий создан (чип с «✕»)');

  // 3) Полный «Сброс» (бар с 1 сценарием → ONE_RESET) + confirm (Enter).
  await click(ONE_RESET[0], ONE_RESET[1]);
  await hoverAway();
  await shot('04_reset_dialog');
  console.log('[шаг] диалог полного сброса открыт');
  await enter();
  await hoverAway();
  await shot('05_after_reset');
  console.log('[шаг] сброс подтверждён (Enter)');

  // 4) Повторное создание (пустой бар → EMPTY_PLUS) и удаление «✕» чипа.
  await click(EMPTY_PLUS[0], EMPTY_PLUS[1]);
  await hoverAway();
  await shot('06_scenario_again');
  console.log('[шаг] сценарий создан повторно');
  await click(ONE_SCEN_CLOSE[0], ONE_SCEN_CLOSE[1]);
  await hoverAway();
  await shot('07_delete_dialog');
  console.log('[шаг] диалог удаления сценария открыт');
  await enter();
  await hoverAway();
  await shot('08_after_delete');
  console.log('[шаг] удаление подтверждено (Enter)');

  const diff = (a, b) => {
    const cmd = `python3 ${path.join(REPO, 'scripts/wasm_ui_diff.py')} ${a} ${b}`;
    try {
      return execSync(cmd, { encoding: 'utf8' }).trim();
    } catch (e) {
      return `n/a (${e.message.split('\n')[0]})`;
    }
  };
  const f = (n) => `${OUT}/whatif_${n}.png`;
  console.log('--- ПИКСЕЛЬНЫЕ ДИФФЫ ---');
  console.log(`вход→создан сценарий (02→03): ${diff(f('02_whatif_enter'), f('03_scenario_created'))} px`);
  console.log(`создан→диалог сброса (03→04): ${diff(f('03_scenario_created'), f('04_reset_dialog'))} px`);
  console.log(`диалог→после сброса (04→05): ${diff(f('04_reset_dialog'), f('05_after_reset'))} px`);
  console.log(`после сброса ≈ вход (05 vs 02): ${diff(f('05_after_reset'), f('02_whatif_enter'))} px`);
  console.log(`повторный сценарий (05→06): ${diff(f('05_after_reset'), f('06_scenario_again'))} px`);
  console.log(`диалог удаления (06→07): ${diff(f('06_scenario_again'), f('07_delete_dialog'))} px`);
  console.log(`после удаления ≈ вход (08 vs 02): ${diff(f('08_after_delete'), f('02_whatif_enter'))} px`);
} finally {
  console.log('--- LOGS (последние 10) ---');
  for (const l of logs.slice(-10)) console.log(l);
  await browser.close().catch(() => {});
  stop();
}
console.log('SCENARIO_DONE');
