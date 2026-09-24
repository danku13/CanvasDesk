# WASM-тестирование UI — рецепт быстрой настройки

Рецепт самопроверки UI на веб-сборке агентом без Windows и GUI (правило в
`AGENTS.md` → «Самопроверка UI на WASM»). Цель — новая сессия воспроизводит
стенд за ~10 минут по готовым командам, а не переоткрывает путь заново.
Прецедент: фикс 52027bc «панели закрываются при клике на себя» проверен
на wasm-сборке кликами в браузере с пиксельными диффами (worklog 2026-09-25).

---

## 1. Уровни проверки — от дешёвого к полному

| Уровень | Что даёт | Когда достаточен |
|---|---|---|
| **L0** — компиляция под wasm32-unknown-unknown | ловит 90 % регрессов (cfg, фичи, типы web-sys/wgpu) | почти всегда для правок ядра/крейтов |
| **L1** — юнит-тесты ядра/моста в wasmtime (wasm32-wasip1) | исполнение чистой логики в wasm-рантайме | правки canvas-core / canvas-mcp |
| **L2** — браузерный стенд: Chromium + WebGPU (SwiftShader) под Xvfb | ПОЛНОЦЕННАЯ UI-проверка: клики, клавиатура, пиксельные диффы | **обязателен для UI-изменений** (раскладка, ввод, панели, hit-тесты, рендер) |
| **L3** — `trunk serve` / `scripts/web_bundle.sh --release` / CI (`wasm-check`, `pages-web`) | эталонный пайплайн, размер бандла | предрелизная приёмка |

Нативные юнит-тесты (cargo test) не отменяются — они закрывают логику; L2
закрывает поведение на web-платформе, где вход (pointer/keyboard), координаты
и композитинг работают иначе, чем в winit.

## 2. Среда агента (проверено 2026-09-25)

**Предустановлено:**

| Инструмент | Где | Зачем |
|---|---|---|
| rust stable + `~/.cargo/bin` | PATH добавить `$HOME/.cargo/bin` | сборка |
| таргет `wasm32-unknown-unknown` | rustup | L0/L2 |
| `wasm-bindgen-cli` **0.2.127** | `~/.cargo/bin/wasm-bindgen` | L2 (версия = крейт в Cargo.lock) |
| node 24 + модуль playwright | `/home/z/.npm-global/lib/node_modules/` | L2-сценарии (npm-плейврайт, `createRequire` от этого корня) |
| python3-playwright | pip | альтернатива для сценариев (`scripts/web_smoke.py`) |
| Chromium 1243 (+ headless shell) | `~/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome` | браузер |
| Xvfb | система | виртуальный дисплей (headed-рендер); обёртка `xvfb-run` в среде не работает — нет `xauth` |
| python3 + PIL + numpy | pip | пиксельный дифф (`scripts/wasm_ui_diff.py`) |

**Отсутствует (ставить по необходимости):**

- `wasmtime` (L1): `curl https://wasmtime.dev/install.sh -sSf | bash`
  (бинарь в `~/.local/bin` — добавить в PATH) + `rustup target add wasm32-wasip1`;
  затем `scripts/wasm_gate.sh` / `scripts/mcp_wasm_gate.sh` без `--check`.
- `trunk` (L3): в этой среде не ставился — prebuilt-ассеты release'ов GitHub
  отдают 404 (как в `pages-web.yml` для wasm-bindgen); `cargo install trunk
  --locked` собирается из исходников (~5–10 мин). Не блокер: ручная сборка
  ниже даёт идентичный результат (тот же bindgen-шаг, что у trunk).

**Особенности среды (важно):**

- Фоновые процессы НЕ выживают между bash-вызовами → http-сервер запускается
  из самого сценария (ребёнок) и гасится в конце; либо всё в одном вызове.
- Dev-wasm весит ~34 МБ → загрузка страницы до 90 с, первый кадр ждать ~7 с.
- Диск песочницы мал → для полного `wasm_gate.sh` чистить
  `target/wasm32-unknown-unknown/incremental` (прецедент FR-057/059).

## 3. Уровень L2 — пошагово

### Шаг 1. Собрать стенд

`trunk` не нужен — bindgen вручную (эквивалент rust-пайплайна trunk):

```bash
cd <корень репо> && export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p canvas-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir target/dist \
    target/wasm32-unknown-unknown/debug/canvas_web.wasm
cp crates/canvas-web/index.html target/dist/
# Инъекция init-глю (то, что trunk делает сам):
python3 - target/dist/index.html <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); html = p.read_text(encoding="utf-8")
if "canvas_web.js" not in html:
    p.write_text(html.replace("</body>", """  <script type="module">
  import init from './canvas_web.js';
  init();
</script>
</body>""", 1), encoding="utf-8")
PY
```

Всё это делает одна команда: **`scripts/wasm_ui_test.sh`** (повторный запуск —
`scripts/wasm_ui_test.sh --no-build`, переиспользует `target/dist`).

### Шаг 2. Запустить сценарий

Сценарий сам поднимает `http.server` на стенде и гасит его; Xvfb поднимает
скрипт-обёртка:

```bash
# Пример — тест панели «О интерфейсе» (координаты калибруются по скриншоту):
ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh --no-build
# (скрипт сам поднимает Xvfb и гасит его по выходу; дисплеи 90–99)
```

Что происходит внутри (`scripts/wasm_ui_scenario.mjs`):

1. Chromium (playwright, **headed**) под Xvfb, который поднимает сам
   `wasm_ui_test.sh` (xvfb-run в среде ломается — нет xauth; Xvfb вручную
   на свободном дисплее 90–99).
2. Флаги (критичны, см. §4): `--enable-unsafe-webgpu --enable-features=Vulkan
   --use-vulkan=swiftshader --use-webgpu-adapter=swiftshader --no-sandbox`.
3. Загрузка → ждём `body > canvas` (winit создаёт канвас при `create_window`)
   + 7 с на первый кадр.
4. Шаги сценария по env-параметрам: `SKIP` (пропустить онбординг),
   `HELP` («?»), `ITEM` (пункт меню), `BODY` (точка ТЕЛА панели — паддинг вне
   интерактивных rect'ов), `BACKDROP` (фон вне панели). Каждый клик —
   скриншот.
5. Пиксельные диффы (`scripts/wasm_ui_diff.py`, PIL): до/после клика по телу
   (~0 — панель осталась) и до/после клика по фону (сотни тысяч px — панель
   закрылась). Оракул — количество изменённых пикселей, не картинка.

### Шаг 3. Прочитать оракулы

- Консоль браузера: `[render] renderer инициализирован backend=BrowserWebGpu`
  — рендер жив; строка `[canvas-web compat] лимиты не распознаны…` — норма
  (шим `index.html` против `maxInterStageShaderComponents`).
- `pageerror` не ожидается; любые panic-строки — регресс.
- Диффы скриншотов — как в шаге 2; дифф ~3–4 тыс. px от «залипшего» hover
  кнопки — косметика доставки кадров (кадр не перерисовался по mouse-move),
  к логике отношения не имеет (прецедент в worklog).

## 4. Грабли (все проверены на практике)

1. **Headless Chromium НЕ композитит WebGPU-канвас в captureScreenshot** —
   контрольный clear-кадр невидим. Только headed-браузер под Xvfb (живой
   композитор). Это главная ловушка уровня L2.
2. **Без SwiftShader-флагов** `request_adapter` → None («GPU-адаптер не
   найден»). Причём у свежих Chromium запрос целиком отклоняется из-за
   лимита `maxInterStageShaderComponents` (удалён из спеки) — шим в
   `index.html` его снимает; без него падение маскируется под «нет GPU».
3. **Версия `wasm-bindgen` CLI обязана совпадать с крейтом в `Cargo.lock`**
   (сейчас 0.2.127) — иначе JS-глю не совместим с wasm-модулем. Проверка:
   `wasm-bindgen --version` vs `rg 'name = "wasm-bindgen"' -A1 Cargo.lock`.
4. **Координаты кликов** — CSS-px вьюпорта (1280×800 = окно канваса);
   DOM-тулбар (верх-право, `#w6-toolbar`) перекрывает верх канваса — кнопка
   «?» кликается по нижней части. Пункты GPU-меню — только по канвасным
   координатам, калибруются по скриншоту шага.
5. **Hover-артефакты в диффе**: курсор ставится в целевую точку ДО
   контрольного кадра (+400 мс), иначе дифф ловит смену hover, а не событие.
6. **Загрузка**: `waitUntil: 'load'` мало — ждать селектор `body > canvas`
   (timeout 90 с) + паузу на первый кадр; dev-сборка тяжёлая.
7. **Сервер стенда** — только внутри процесса сценария (см. §2, фоновые
   процессы); порт по умолчанию 8081.

## 5. Что проверено этим рецептом (хронология)

- 2026-09-25: фикс 52027bc — UI-консоль и «О интерфейсе» не закрываются по
  клику на своё тело, закрываются по фону (backdrop-контракт), Esc жив.
  Скриншоты и диффы — в worklog репо; сценарии-предки текущего
  `wasm_ui_scenario.mjs` — `wasm_probe*.mjs` (сессия агента).
- 2026-09-25 (тот же день): сам рецепт закоммичен и проверен целиком
  (`ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh`):
  стенд собрался, `backend=BrowserWebGpu`, клик по телу — 3 697 px
  (hover-косметика, панель осталась), клик по фону — 552 616 px (панель
  закрылась). Из прогона выловлены две грабли и вписаны выше: xvfb-run без
  xauth (Xvfb вручную) и http.server-ребёнок, держащий event loop node
  (явный `kill` в finally + watchdog).

## 6. Шпаргалка

```bash
# L0 — всегда
scripts/wasm_gate.sh --check                 # или cargo check --target wasm32-unknown-unknown -p <крейт>

# L1 — единожды ставится wasmtime, затем
scripts/wasm_gate.sh && scripts/mcp_wasm_gate.sh

# L2 — UI-проверка (стенд + сценарий)
ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh
scripts/wasm_ui_test.sh --no-build           # повторно, без пересборки

# L3 — релизный бандл / дев-сервер (если trunk доступен)
(cd crates/canvas-web && trunk serve)        # http://127.0.0.1:8080
scripts/web_bundle.sh --dist _site/app       # как в CI pages-web
```

Когда L2 недоступен (среда без node/playwright/Xvfb) — правило из
`AGENTS.md`: явно доложить «WASM-проверка не выполнялась, причина» и дать
владельцу ручную инструкцию (что открыть, куда кликнуть, что считается
успехом).
