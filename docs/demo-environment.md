# Стенд записи визуальных демо (Linux, headless, без root)

Как максимально быстро подготовить среду для записи демо CanvasDesk:
живое окно приложения в виртуальном X-сервере, программное управление
мышью/клавиатурой, запись GIF/MP4 и скриншоты. Проверено в контейнере
без GPU и без root (9 скриншотов + GIF 1.2 МБ + MP4 за один прогон).

Всё нужное лежит в репозитории:

```
scripts/demo/
├── env.sh          # bootstrap зависимостей (идемпотентен, без root)
├── demo_run.sh     # полный прогон: Xvfb + запись + сценарий → GIF/MP4/PNG
├── demo_driver.py  # референсный сценарий (xdotool-примитивы + хронометраж)
└── close_window.py # штатное закрытие WM_DELETE_WINDOW (канвас сохранится)
```

> **Родственный рецепт:** [docs/DEMO.md](DEMO.md) — ручной вариант стенда
> (XTEST-автоматизация на python-xlib через `scripts/xdemo.py`, дисплей
> 1600×1000, эталонная сцена `examples/demo-numi.canvas`, свои шаги
> развёртки). Выбирайте его, когда нужна посимвольная интерактивная
> подготовка сцены; этот документ — автоматизированный вариант «одним
> прогоном» с готовым конвейером GIF/MP4.

## 1. TL;DR — от нуля до GIF за 4 команды

```bash
git clone https://github.com/danku13/CanvasDesk && cd CanvasDesk
cargo build --release -p canvas-app
scripts/demo/demo_run.sh
# → demo-out/: canvasdesk-demo.gif, canvasdesk-demo.mp4, 01..08-*.png
```

`demo_run.sh` сам подключает `env.sh` (тот докачивает недостающие deb-пакеты
в `~/.cache/canvasdesk-demo` и экспортирует переменные), поднимает Xvfb,
включает запись, запускает приложение и сценарий, затем собирает GIF и MP4.

Системные требования: bash, python3, cargo, `ffmpeg`, `Xvfb` (пакет
`xvfb`). Если их нет и нет root — см. §3.

## 2. Как устроен стенд

| Компонент | Роль | Почему именно он |
|---|---|---|
| Xvfb (`:99`, 800×600×24) | виртуальный X-сервер | настоящий дисплей не нужен |
| Vulkan **lavapipe** | софт-рендер для wgpu | в контейнере/CI нет GPU; wgpu работает через Vulkan ICD `lvp_icd.json` из mesa-vulkan-drivers |
| `VK_DRIVER_FILES` (+алиас `VK_ICD_FILENAMES`) | указывает загрузчику на lavapipe | ICD не установлен в систему — грузим по явному пути |
| `LD_LIBRARY_PATH` | подмешивает извлечённые библиотеки | никакие пакеты не ставятся в систему |
| xdotool (XTEST) | мышь/клавиатура сценария | libxdo синхронизируется с X сам; голый python-Xlib/XTEST деградирует после асинхронных X-ошибок |
| ffmpeg `x11grab` | непрерывная запись + одиночные кадры (скриншоты) | одна утилита и для видео, и для PNG |
| `palettegen`/`paletteuse` | GIF в два прохода | однопроходная палитра даёт артефакты на плоских цветах egui |

Проверка, что рендер пошёл через lavapipe: в логе приложения
(`demo-out/../run/app.log`, строка adapter) должно быть `llvmpipe`.

## 3. Зависимости без root (env.sh)

`env.sh` скачивает только недостающие пакеты (`apt-get download` — без
установки) и распаковывает `dpkg -x` в `~/.cache/canvasdesk-demo/`
(переопределяется `CANVASDESK_DEMO_BASE`):

| Пакет | Что даёт | Куда распаковывается |
|---|---|---|
| `mesa-vulkan-drivers` | `lvp_icd.json` + `libvulkan_lvp.so` (lavapipe) | `mesa-vk/` |
| `libvulkan1` | загрузчик Vulkan (обычно уже есть в системе) | `syslibs/` |
| `libxkbcommon-x11-0`, `libxcb-xkb1` | клавиатура winit под X11 | `syslibs/` |
| `xdotool`, `libxdo3`, `libxtst6` | XTEST-управление | `syslibs/` |

Скрипт идемпотентен: повторный вызов мгновенно переиспользует кэш. Если
`apt-get download` падает (устаревшие списки пакетов) — один раз
`sudo apt-get update`, либо скачать `.deb` вручную с packages.ubuntu.com
в `~/.cache/canvasdesk-demo/debs/` и перезапустить `env.sh`.

Если `ffmpeg`/`Xvfb`/`python3` отсутствуют и root недоступен — так же
скачать пакеты `ffmpeg`, `xvfb` (плюс их зависимости) и добавить
`usr/bin` в `PATH`. На практике быстрее попросить у окружения root на
одну команду: `apt-get install -y xvfb ffmpeg python3`.

## 4. Запись (demo_run.sh)

```bash
scripts/demo/demo_run.sh                          # референсный сценарий
scripts/demo/demo_run.sh my_scenario.py           # свой сценарий
```

Переменные окружения (все опциональны):

| Переменная | По умолчанию | Смысл |
|---|---|---|
| `CANVASDESK_APP` | `<repo>/target/release/canvasdesk` | путь к бинарю |
| `DEMO_DISPLAY` | `:99` | номер X-дисплея |
| `DEMO_SIZE` | `800x600` | разрешение окна и записи |
| `FPS` / `GIF_FPS` | `15` / `9` | частота записи / GIF |
| `CANVASDESK_DEMO_OUT` | `<repo>/demo-out/` | куда класть артефакты |
| `CANVASDESK_DEMO_SCRATCH` | `~/.cache/canvasdesk-demo/run` | рабочая папка прогонов |

Конвейер: Xvfb → ffmpeg (промежуточный lossless `libx264rgb crf=0`) →
приложение → драйвер сценария → стоп-запись → GIF (`fps=9`,
`palettegen=stats_mode=diff` + `paletteuse=dither=bayer:bayer_scale=4`)
и MP4 (`h264 crf=22 yuv420p +faststart` — совместим с мессенджерами).
Отдельные PNG-скриншоты делает сам драйвер (см. ниже).

## 5. Адаптация сценария (demo_driver.py)

Копируйте файл, меняйте координаты — примитивы уже готовы:

- `click/dblclick(x, y)`, `key("ctrl+f")`, `type_str("1200")`,
  `ctrl_key("z")`, `wheel(n, up=, ctrl=)` — базовые действия;
- `drag(x1, y1, shift=, steps=)` — drag с промежуточными точками
  (приложение должно видеть плавное движение, иначе не сработает
  создание рёбер и рамка выделения);
- `pan(x1, y1)` — Space+drag; перед любым drag/pan делайте явный
  `move(x, y)` к стартовой точке (`--sync` — камера не «уедет»);
- `shot("name.png", dwell=0.5)` — скриншот кадром x11grab;
- `wait_window()` — ждёт окно «CanvasDesk» и фокусирует его.

Геометрия: при камере по умолчанию (0,0, zoom=1) экранные координаты
`screen = world + (400, 300)` (центр окна — мировая точка (0,0)). Порты
ноды — середины сторон: Right = `(x+w, y+h/2)`, Left = `(x, y+h/2)`,
в screen-координатах сдвиг на (400,300). В референсном сценарии эти
выкладки прокомментированы прямо на местах.

## 6. Грабли (всё проверено на реальном демо)

1. **Enter в редакторе заметки — это коммит текста; Ctrl+Enter — новая
   строка.** Если заметку не закоммитить, редактор остаётся открытым,
   hover гасится, и следующий drag ноды ломается (press уходит в drag
   редактора). После `type_str` всегда `key("Return")`.
2. **press/drop value-ребра — строго ВНУТРИ нод (~5 px от края).** На
   самой границе spatial hit не находит ноду, press уходит в рамку
   выделения — ребро не создаётся и сценарий молча едет дальше.
3. **Явный `move` перед pan/drag.** Без стартовой точки камера едет с
   прошлого действия, все последующие координаты «плывут».
4. **F3 (HUD) нажимать до Ctrl+F.** Если поисковая строка уже содержит
   запрос, F3 циклит результаты поиска вместо показа HUD.
5. **Не использовать голый python-Xlib/XTEST** как основной драйвер:
   после асинхронной X-ошибки (например, BadRRMode от Xvfb) события
   начинают молча теряться. xdotool/libxdo устойчив.
6. **Прогрев рендера ~4 c.** Первый скриншот сразу после `wait_window`
   будет пустым/чёрным — в сценарии стоит `time.sleep(4.0)` после
   появления окна.
7. **GIF только в два прохода** (`palettegen stats_mode=diff` →
   `paletteuse dither=bayer`): однопроходная палитра полосит плоские
   цвета egui, а без dither заметны лестницы на полупрозрачных тенях.
8. **Логи wgpu тонут в спаме лавапайпа** — `RUST_LOG=info,wgpu_hal=warn,
   wgpu_core=warn` (env.sh ставит по умолчанию); в app.log ищите
   `adapter=llvmpipe` и паники.
9. **Закрывать приложение штатно** (`close_window.py` →
   WM_DELETE_WINDOW), иначе финал демо без сохранения канваса. Скрипту
   нужен `python3-xlib`; без него (или при ошибке Xlib) он откатывается
   на `xdotool windowclose`. Важно: `ClientMessage` собирать только
   ключевыми словами — сигнатуры позиционных аргументов различаются
   между версиями python-xlib (TypeError на новых).
10. **Не убирайте `source env.sh` из demo_run.sh.** Без `LD_LIBRARY_PATH`
    приложение падает на старте: `xkbcommon-dl` запрашивает и версионные,
    и безверсионные имена (`libxkbcommon-x11.so`) — env.sh создаёт
    недостающие symlink'и после распаковки deb. Симптом: паника
    «Library libxkbcommon-x11.so could not be loaded».
11. **stdout фоновых потомков — в файл, не в stdout скрипта.** Если
    Xvfb/ffmpeg/приложение наследуют stdout, то конструкция
    `demo_run.sh | tail` зависает после завершения скрипта: живой
    потомок удерживает пайп. В demo_run.sh перенаправления уже стоят.

## 7. Запись в CI (GitHub Actions)

Стенд стартует и на раннере без изменений — `ubuntu-latest` уже содержит
ffmpeg/xvfb-библиотеки, а env.sh сам докачает lavapipe:

```yaml
- run: cargo build --release -p canvas-app
- run: sudo apt-get install -y xvfb ffmpeg python3-xlib   # если чего-то нет
- run: scripts/demo/demo_run.sh
- uses: actions/upload-artifact@v4
  with: { name: demo, path: demo-out/ }
```

## 8. Чеклист перед новым демо

- [ ] `cargo build --release -p canvas-app` собрался, тесты зелёные
- [ ] сценарий скопирован из `demo_driver.py`, координаты пересчитаны (§5)
- [ ] `scripts/demo/demo_run.sh my_scenario.py` прошёл, `driver_rc=0`
- [ ] в app.log: `adapter=llvmpipe`, нет паник; в GIF видно все шаги
- [ ] GIF просмотрен целиком (дёрганая анимация = увеличить dwell/шаги drag)
- [ ] артефакты из `demo-out/` сложены в итоговую папку / приложены к PR
