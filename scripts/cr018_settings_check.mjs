// Приёмка фикса настроек: dropdown открыт — строки таба не исчезают.
import { createRequire } from 'module';
const NPM_ROOT = process.env.NPM_ROOT ?? '/home/z/.npm-global/lib/node_modules/';
const { chromium } = createRequire(NPM_ROOT)('playwright');
const OUT = process.env.OUT ?? '/home/z/my-project/repro_settings';
const URL = process.env.REPRO_URL ?? 'http://127.0.0.1:8093/index.html';
const CHROME = '/home/z/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome';

const browser = await chromium.launch({
  headless: false,
  executablePath: CHROME,
  args: ['--no-sandbox', '--disable-gpu-sandbox', '--enable-unsafe-webgpu',
    '--enable-features=Vulkan', '--use-vulkan=swiftshader',
    '--use-webgpu-adapter=swiftshader', '--window-size=1280,800'],
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const shot = (n) => page.screenshot({ path: `${OUT}/${n}.png` });
  const click = async (x, y, ms = 800) => {
    await page.mouse.move(x, y); await page.waitForTimeout(120);
    await page.mouse.down(); await page.waitForTimeout(60);
    await page.mouse.up(); await page.waitForTimeout(ms);
  };
  await page.goto(URL, { waitUntil: 'load' });
  await page.waitForSelector('body > canvas', { timeout: 90000 });
  await page.waitForTimeout(6000);
  await click(649, 395, 1200);
  await click(811, 299, 1000);
  await click(729, 467, 1200);
  // ⚙ — правый верхний кластер
  await click(1250, 67, 1500);
  await shot('40_settings_general');
  // Dropdown первой строки (Позиция летающей кнопки): контрол справа
  await click(820, 244, 1200);
  await shot('41_dropdown_open');
  // Выбор другого пункта — применение + закрытие
  await click(800, 180, 1200);
  await shot('42_dropdown_applied');
} finally {
  await browser.close().catch(() => {});
}
