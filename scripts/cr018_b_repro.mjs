// CR-018 v1.1: репро владельца на живом Pages (ff2431a).
// Шаги: онбординг → пустой холст → палитра → auth-service → скриншоты
// (норма / правка / после правки) → настройки «Общие» + dropdown.
import { createRequire } from 'module';
import { spawn } from 'child_process';
import { mkdirSync } from 'fs';

const REPO = '/home/z/my-project/CanvasDesk';
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = process.env.OUT ?? '/home/z/my-project/repro_out';
const URL = process.env.REPRO_URL ?? 'https://danku13.github.io/CanvasDesk/app/';
mkdirSync(OUT, { recursive: true });
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';

const browser = await chromium.launch({
  headless: false,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  const shot = (n) => page.screenshot({ path: `${OUT}/${n}.png` });
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await page.waitForTimeout(120);
    await page.mouse.down(); await page.waitForTimeout(60);
    await page.mouse.up(); await page.waitForTimeout(ms);
  };
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(6000);
  await shot('00_onboarding');
  // Онбординг: продолжить без роли → пропустить тур → пустой холст
  await click(649, 395, 1200);
  await shot('01_after_role');
  await click(811, 299, 1000);
  await shot('02_after_skip');
  await click(729, 467, 1500);
  await shot('03_empty_canvas');
  // Палитра: чип категории слева (раскрыть док)
  await click(83, 415, 600);
  await shot('04_palette_try1');
  await click(83, 442, 1200);
  await shot('05_palette_try2');
  // Поиск палитры (Ctrl+P → фокус в поиск) + печать «аутент»
  await page.keyboard.press('Control+p'); await page.waitForTimeout(600);
  await shot('06_palette_ctrlp');
  await page.keyboard.type('аутент', { delay: 30 });
  await page.waitForTimeout(1200);
  await shot('07_palette_search');
  console.log('[шаг] палитра готова — дальше вручную по скриншотам');
} finally {
  await browser.close().catch(() => {});
  for (const l of logs) if (l.includes('pageerror') || l.includes('паника')) console.log(l);
}
