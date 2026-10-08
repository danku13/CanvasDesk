// CR-027: фит при DPR 1.5 — без онбординга (сеем config), карточка — по
// синей рамке выделения. Сравнение роста кромки с DPR 1.
import { createRequire } from 'module';
import { mkdirSync } from 'fs';

const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const URL = 'https://danku13.github.io/CanvasDesk/app/';
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';
mkdirSync('/home/z/my-project/repro_out_dpr3', { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Карточка по синей рамке выделения (B >> R): bbox синих пикселей + текст
// (светлые пиксели) ниже нижней кромки.
async function cardProbe(page) {
  return page.evaluate(() => {
    const c = document.querySelector('body > canvas');
    if (!c) return null;
    const off = document.createElement('canvas');
    off.width = c.width; off.height = c.height;
    const ctx = off.getContext('2d');
    ctx.drawImage(c, 0, 0);
    const k = c.width / c.clientWidth;
    const W = c.clientWidth, H = c.clientHeight;
    const step = 2; // прорежение скана
    let minX = 1e9, maxX = -1, minY = 1e9, maxY = -1, blue = 0;
    const img = ctx.getImageData(0, 0, c.width, c.height).data;
    const at = (x, y) => {
      const i = (Math.round(y * k) * c.width + Math.round(x * k)) * 4;
      return [img[i], img[i + 1], img[i + 2]];
    };
    for (let y = 0; y < H; y += step) {
      for (let x = 0; x < W; x += step) {
        const p = at(x, y);
        if (p[2] > 160 && p[2] - p[0] > 70 && p[2] - p[1] > 40) {
          blue++;
          if (x < minX) minX = x; if (x > maxX) maxX = x;
          if (y < minY) minY = y; if (y > maxY) maxY = y;
        }
      }
    }
    let textBelow = 0;
    if (maxY > 0) {
      for (let y = maxY + 3; y < Math.min(maxY + 44, H); y += 1)
        for (let x = minX; x < maxX; x += 2) {
          const p = at(x, y);
          if (p[0] > 150 && p[1] > 150 && p[2] > 150) textBelow++;
        }
    }
    return blue > 40 ? { minX, maxX, minY, maxY, w: maxX - minX, h: maxY - minY, textBelow } : { none: true, blue };
  });
}

async function runCase(browser, label, dsf, outDir) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dsf });
  const shot = (n) => page.screenshot({ path: `${outDir}/${label}_${n}.png` });
  // Онбординг выключен флагом конфига ДО загрузки приложения
  await page.addInitScript(() => {
    try { localStorage.setItem('canvasdesk.config', 'onboarding_done = true'); } catch (e) {}
  });
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await sleep(9000);
  await shot('s0_start');

  // Создание заметки двойным кликом (позиция не важна — ищем рамку)
  await page.mouse.dblclick(560, 340); await sleep(1600);
  await page.keyboard.type('fit', { delay: 40 });
  await page.keyboard.press('Enter'); await sleep(900);
  await shot('s1_body');

  for (let i = 1; i <= 8; i++) {
    await page.keyboard.type(`line${i} test`, { delay: 20 });
    if (i < 8) { await page.keyboard.press('Control+Enter'); await sleep(500); }
    await sleep(250);
    const m = await cardProbe(page);
    console.log(`[${label} строка ${i}] ${JSON.stringify(m)}`);
    if (i === 1 || i === 4 || i === 8) await shot(`f${String(i).padStart(2, '0')}`);
  }
  await page.mouse.click(200, 720); await sleep(1000);
  const m = await cardProbe(page);
  console.log(`[${label} после коммита] ${JSON.stringify(m)}`);
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
  await runCase(browser, 'dpr150c', 1.5, '/home/z/my-project/repro_out_dpr3');
  await runCase(browser, 'dpr100', 1, '/home/z/my-project/repro_out_dpr3');
} finally {
  await browser.close().catch(() => {});
}
console.log('ГОТОВО');
