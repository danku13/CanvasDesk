// CR-018 v1.1: репро владельца — часть 2 (вставка auth-service, правка, настройки).
import { createRequire } from 'module';
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = '/home/z/my-project/repro_out';
const URL = 'https://danku13.github.io/CanvasDesk/app/';
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
  await click(649, 395, 1200);   // без роли
  await click(811, 299, 1000);   // пропустить тур
  await click(729, 467, 1500);   // пустой холст
  await click(83, 442, 1400);    // раскрыть палитру
  await click(150, 75, 600);     // фокус в поиск
  await page.keyboard.type('аутент', { delay: 40 });
  await page.waitForTimeout(1200);
  await shot('10_search_results');
  await page.keyboard.press('Enter'); // вставить в центр
  await page.waitForTimeout(2000);
  await shot('11_node_inserted');
  await page.keyboard.press('Escape'); // свернуть палитру
  await page.waitForTimeout(800);
  await shot('12_node_normal');
  // Правка: клик по телу ноды (нижняя треть — строка params)
  await click(640, 560, 1400);
  await shot('13_edit_mode');
  await page.keyboard.type('x = 1', { delay: 30 });
  await page.waitForTimeout(600);
  await shot('14_edit_typed');
  await page.keyboard.press('Escape'); // откат
  await page.waitForTimeout(1000);
  await shot('15_after_edit');
  // Настройки: кнопка ⚙ (правый верхний кластер)
  await click(1250, 67, 1500);
  await shot('16_settings_general');
  // Открыть dropdown первой строки (контрол справа) — ткнуть в него.
  // Позиция уточняется по скриншоту 16; контроль первой строки ≈ правый край.
  await click(1150, 205, 1200);
  await shot('17_settings_dropdown');
} finally {
  await browser.close().catch(() => {});
  for (const l of logs) if (l.includes('pageerror') || l.includes('паника')) console.log(l);
}
