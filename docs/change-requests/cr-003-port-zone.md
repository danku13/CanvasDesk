# CR-003: Регулировка активной зоны портов (оттягивание связей)

- **Статус:** в анализе
- **Тип:** CR
- **Приоритет:** важно
- **Владелец:** агент (реализация)
- **Источник:** сообщение пользователя (сессия 2026-09-13): «нужно научиться регулировать offset активной зоны откуда можно оттягивать связь от одной ноды к другой для удобства и чтобы можно было не целиться при протягивании связей»
- **Связанные задачи:** TASKS.md T8 (связи); CR-002 (хэндлы концов используют ту же зону); docs/interface-objects/node.md §4
- **Создан:** 2026-09-13
- **Обновлён:** 2026-09-13
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Зона захвата порта для старта новой связи фиксирована: `PORT_HIT_PX = 10.0`
экранных px (`crates/canvas-core/src/edgegeom.rs:14`) — в это маленькое кольцо
надо точно попасть курсором. Требуется настройка величины зоны (offset от
порта, внутри которого начинается edge-drag), чтобы не целиться. Выявлено
пользователем.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `node` | Зона портов конфигурируется (10–40 экранных px), видимый кружок порта растёт вслед за зоной (кламп) | `docs/interface-objects/node.md` §4 |
| настройки | Новая строка панели настроек «Зона портов», персистентность в `config.toml` | `crates/canvas-core/src/settings.rs` |
| `canvas-core` | `port_at` получает параметр допуска (константа — дефолт) | `crates/canvas-core/src/edgegeom.rs:442` |
| `canvas-render` | `build_port_instances` рисует кружки по зоне | `crates/canvas-render/src/cards.rs:495` |

## Анализ (Root Cause)

- `port_at` (`crates/canvas-core/src/edgegeom.rs:442-454`) берёт глобальную
  константу `PORT_HIT_PX` (`edgegeom.rs:14`): передать пользовательский допуск
  невозможно — параметра нет.
- `Settings` (`crates/canvas-core/src/settings.rs:148-167`) — поля зоны нет;
  расширение по правилам модуля: новое поле + `#[serde(default)]`.
- Вызов `port_at` в приложении: `main.rs:3273` (старт edge-drag) — значение
  пробрасывать из настроек.
- Рендер портов при hover: `build_port_instances` (`cards.rs:495`) рисует
  `PORT_DOT = 10.0` world-px (`cards.rs:205`) — визуальный отклик не зависит от
  зоны.

## Требуемые изменения (Changes)

1. `canvas-core/src/edgegeom.rs`: `port_at(node, point, zoom, tolerance_px)`;
   `PORT_HIT_PX` остаётся дефолтом (используется тестами и вызовами без настроек).
2. `canvas-core/src/settings.rs`: `port_zone_px: f32` (дефолт 10.0, пресеты
   10/14/20/28/40; кламп в границы при загрузке).
3. `canvas-app/src/lib.rs` (ui): строка панели `SettingsRow::PortZone`
   («Зона портов: N px», клик — цикл по пресетам).
4. `main.rs`: проброс `self.settings.port_zone_px` в `port_at`; сохранение
   настройки (общий хвост `apply_settings_row`).
5. `canvas-render/src/cards.rs`: `build_port_instances(canvas, node, zone)` —
   диаметр кружка `clamp(zone, 10, 26)` world-px (визуальный отклик роста зоны
   без гигантских кружков).

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §4 — параметризованная зона портов.
- `docs/ACCEPTANCE.md` — раздел ручной приёмки CR-003.
- `docs/change-requests/cr-003-port-zone.md` (этот файл) — статусы.

## Проверка (Verification)

- Панель настроек: строка «Зона портов: 10 px» → клики циклят 10→14→20→28→40→10;
  значение сохраняется в `config.toml` (перезапуск — сохраняется).
- Зона 40: edge-drag начинается при захвате заметно в стороне от центра порта
  (кружок порта видимо больше).
- Зона не захватывает соседние ноды: hit-test портов проверяется ПОСЛЕ
  resize-угла и до двойного клика — порядок не меняется.
- `port_at` с дефолтом `PORT_HIT_PX` — прежнее поведение (тесты edgegeom
  не ломаются).
- Unit-тесты: `port_at` с кастомным допуском (близко/далеко/между портами),
  пресеты и клампы Settings, `build_port_instances` с зоной.

## История изменений (Changelog)

- `2026-09-13` — агент: документ создан по запросу пользователя, статус `в анализе`.

## Источники истины (References)

- `crates/canvas-core/src/edgegeom.rs:10-17, 440-454` — `PORT_HIT_PX`, `port_at`.
- `crates/canvas-core/src/settings.rs:144-182` — `Settings` (правило расширения).
- `crates/canvas-app/src/main.rs:3266-3283` — единственный вызов `port_at` в приложении.
- `crates/canvas-render/src/cards.rs:205, 495` — `PORT_DOT`, `build_port_instances`.
