// CR-018 v1.2: репро — раскрытие desc «⋯ целиком ▾» переживает клик вне ноды
// (автосворачивание ретировано). OPFS-сев ?canvas=authdesc → скан-клик по
// второй строке desc до раскрытия → клик МИМО ноды → desc остаётся раскрытым.
import { createRequire } from 'module';
import { readFileSync } from 'fs';
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = process.env.OUT ?? '/home/z/my-project/repro_v12';
const URL = process.env.REPRO_URL ?? 'http://127.0.0.1:8080/?canvas=authdesc';
const CANVAS_JSON = readFileSync('/home/z/my-project/auth_desc.canvas', 'utf8');
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
  // Фингерпринт полосы ниже тела (раскрытие desc сдвигает ИТОГ вниз):
  // длина PNG клипа — стабильный дифф состояния.
  const stripFp = () =>
    page.screenshot({ clip: { x: 410, y: 270, width: 400, height: 40 } })
      .then((b) => b.length);
  await page.addInitScript(async (content) => {
    try {
      const root = await navigator.storage.getDirectory();
      const fh = await root.getFileHandle('authdesc.canvas', { create: true });
      const w = await fh.createWritable();
      await w.write(content);
      await w.close();
    } catch (e) { console.error('[seed] OPFS fail', e); }
  }, CANVAS_JSON);
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(7000);
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await page.waitForTimeout(1500);
  // Селект ноды по шапке — фиксируем состояние dim/selection ДО базы
  // (иначе селект-клик скана даёт ложную смену фингерпринта)
  await click(600, 112, 1000);
  await shot('40_desc_clamped');
  const baseFp = await stripFp();
  // Экспандер «⋯ целиком ▾» — ОТДЕЛЬНАЯ строка под клампом desc (y≈189,
  // x≈408..492): скан слева направо до первого перехода состояния
  let hitX = null;
  for (let x = 400; x <= 540 && hitX === null; x += 15) {
    await click(x, 189, 700);
    const fp = await stripFp();
    if (fp !== baseFp) hitX = x;
  }
  await shot('41_desc_expanded');
  console.log('EXPANDER_HIT_X=' + hitX);
  if (hitX !== null) {
    // Клик МИМО ноды (пустой канвас) — раскрытие должно сохраниться
    await click(180, 620, 1200);
    await shot('42_after_outside_click');
    const fpAfter = await stripFp();
    console.log('FP_EXPANDED_vs_AFTER=' + fpAfter);
    // Стык фич: правка при РАСКРЫТОМ desc — буфер стартует ниже полного
    // desc (вариант B), ряды редактора 1:1 с рядами раскрытого кадра
    await click(600, 112, 900);      // селект по шапке
    await click(560, 362, 1500);     // клик по строке token_verify → правка
    await shot('43_edit_expanded_desc');
    await page.keyboard.press('Escape');
  }
} finally {
  await browser.close().catch(() => {});
  for (const l of logs) if (l.includes('pageerror') || l.includes('[seed]') || l.includes('паника')) console.log(l);
}
