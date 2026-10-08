// CR-027: фит при deviceScaleFactor≠1 (Windows 125%/150%) и при зуме канваса≠1.
import { createRequire } from 'module';
import { mkdirSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const URL = 'https://danku13.github.io/CanvasDesk/app/';
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
mkdirSync('/home/z/my-project/repro_out_dpr', { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function cardBottom(page, rx, ry) {
  return page.evaluate(({ rx, ry }) => {
    const c = document.querySelector('body > canvas');
    if (!c) return null;
    const off = document.createElement('canvas');
    off.width = c.width; off.height = c.height;
    const ctx = off.getContext('2d');
    ctx.drawImage(c, 0, 0);
    const scale = c.width / c.clientWidth;
    const px = (x, y) => {
      const d = ctx.getImageData(Math.round(x * scale), Math.round(y * scale), 1, 1).data;
      return [d[0], d[1], d[2]];
    };
    const ref = px(rx, ry);
    const near = (a, b) => Math.abs(a[0] - b[0]) <= 12 && Math.abs(a[1] - b[1]) <= 12 && Math.abs(a[2] - b[2]) <= 12;
    let bottom = -1;
    for (let y = ry; y < c.clientHeight; y++) if (near(px(rx, y), ref)) bottom = y;
    let textBelow = 0;
    for (let y = bottom + 3; y < Math.min(bottom + 40, c.clientHeight); y++)
      for (let x = rx - 120; x < rx + 120; x++) {
        const p = px(x, y);
        if (p[0] > 150 && p[1] > 150 && p[2] > 150) textBelow++;
      }
    return { bottom, textBelow };
  }, { rx, ry });
}

async function runCase(browser, label, dsf, zoomSteps, outDir) {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: dsf,
  });
  const shot = (n) => page.screenshot({ path: `${outDir}/${label}_${n}.png` });
  const sleepL = sleep;
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await sleepL(8000);
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await sleepL(120);
    await page.mouse.down(); await sleepL(60);
    await page.mouse.up(); await sleepL(ms);
  };
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await click(729, 467, 1500);
  // Зум канваса (если задан): ctrl+wheel
  for (let i = 0; i < Math.abs(zoomSteps); i++) {
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, zoomSteps > 0 ? -120 : 120);
    await page.keyboard.up('Control');
    await sleepL(300);
  }
  // Создание заметки в центре видимого холста
  await page.mouse.dblclick(640, 380); await sleepL(1500);
  await page.keyboard.type('fit', { delay: 40 });
  await page.keyboard.press('Enter'); await sleepL(900);

  for (let i = 1; i <= 8; i++) {
    await page.keyboard.type(`line${i} test`, { delay: 20 });
    if (i < 8) { await page.keyboard.press('Control+Enter'); await sleepL(500); }
    await sleepL(300);
    const m = await cardBottom(page, 760, 430);
    console.log(`[${label} строка ${i}] кромка=${m?.bottom} textBelow=${m?.textBelow}`);
    await shot(`f${String(i).padStart(2, '0')}`);
  }
  await click(250, 650, 1200);
  const m = await cardBottom(page, 760, 430);
  console.log(`[${label} после коммита] кромка=${m?.bottom} textBelow=${m?.textBelow}`);
  await shot('f09_committed');
  await page.close();
}

const browser = await chromium.launch({
  headless: true,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
try {
  // A) DPR 1.5 (Windows 150%) — зум канваса 0
  await runCase(browser, 'dpr150', 1.5, 0, '/home/z/my-project/repro_out_dpr');
  // B) DPR 1, зум канваса -3 (уменьшение колесом с Ctrl)
  await runCase(browser, 'zoomout3', 1, -3, '/home/z/my-project/repro_out_dpr');
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО');
