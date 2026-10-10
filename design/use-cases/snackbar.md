# Снэк-бар (Snackbar)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/snackbar.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Пассивное уведомление с опциональным действием: «Удалено — Undo». Отличие от
тоста (`../use-cases/toast.md`): тост — только факт, не кликабелен (Passive,
L7); снэк-бар несёт action-кнопку (интерактивную зону) и тон. Спека Wave C
(`docs/dev-researches/ui-kit-deep-review-2026-10.md` §5.3.14): «toast_area
есть (позиционирование), но без action-button и без proper state-machine».

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Структура | message + optional action, единая pill |
| TTL | ttl_ms, default 3000 (= TOAST_TTL_MS) |
| Позиция | bottom-center: y = viewport.bottom() − bar_h − 24 (SPACING_XL) |
| bar_h, pad_h | параметры потребителя (тесты модуля: 36 = ControlSize::Md, 12 = SPACING_LG) |
| Ширина | msg_w + 2·pad_h + gap + action_w; gap = pad_h при наличии action |
| Action | ширина = замер метки (TextMeasurer) + 2·pad_h; rect справа от сообщения |
| Радиус | Shape::L (12 = RADIUS_PILL) |
| Тень | Elevation::Md (offset 4, blur 8, α 0.16) |

## 3. Токены (слоты KitPalette)
`snackbar_style(tone)` — тон → слот без арифметики (K2,
`../rules/10-components.md` §K2):
| BadgeTone | Слот |
|---|---|
| Default | panel_fill |
| Primary | control_primary |
| Success | control_success |
| Warning | control_warning |
| Danger | control_danger |
| Info | accent |

Текст — text_title (on-fill), рамка отключена ([0;4]).

## 4. Состояния
ST1-состояний у контейнера нет; action-кнопка — контрол потребителя с матрицей
ST1 (`../rules/08-states.md`). Жизненный цикл: push (expire = now_ms +
ttl_ms) → активен → tick удаляет истёкшие (retain expire > now_ms). Очередь
FIFO (`Vec`), стакается вертикально bottom-center; геометрию стопки и фейд
строит потребитель (M3.5, `../rules/06-motion.md`).

## 5. Взаимодействие
- Клик по action_rect — контракт ST3 (`../rules/08-states.md`): press внутри +
  release внутри; обработчик — потребитель (модуль возвращает rect).
- Клик мимо снэк-бара: close-on-outside в модуле не реализован — политика
  поверхности потребителя; для интерактивного снэк-бара Passive тоста
  недостаточно — решение слоя за правкой `../rules/05-layering.md`.
- Автозакрытие — только ttl (`tick`); ручного dismiss-API нет.
- Клавиатура: action участвует в FocusRing поверхности (Tab-кольцо, L4).

## 6. Граничные случаи
- Ширина больше вьюпорта: центрирование клампит левый край (max(0.0) →
  x = viewport.x); текст НЕ усекается — ellipsis из док-комментария
  `SnackbarLayout.label` («после ellipsis») не реализован: расхождение
  комментария с кодом — задача рефакторинга.
- Очередь без лимита: каждый push добавляет элемент; дедупликации подряд
  (как у тоста — идентичный текст заменяет верхний) нет.
- expire фиксируется на push; ttl не пересчитывается при показе.
- Тон Danger/Warning поверх сообщений кита — тот же слот на светлой теме
  (не дифференцирован) — I-1.

## 7. Где в коде
`crates/canvas-ui/src/component/snackbar.rs` — `snackbar_layout`,
`snackbar_style`, `Snackbar`, `SnackbarQueue` (push/tick), тесты очереди —
в модуле. Фасад — `crates/canvas-ui/src/kit.rs` (122–123). Потребителей в
`crates/canvas-app` нет; секции витрины в `crates/canvas-app/src/kit_ui.rs`
нет — долг K1.1/K4.

## 8. Что меняется при правке файла
- Геометрия/TTL → модуль + юнит-тесты; подача (стопка, фейд) — потребитель.
- Новый тон → K2.1 одновременно: KitPalette + мост From<&ThemeColors> +
  таблица K2 (`../rules/10-components.md`).
- Появление dismiss/close-on-outside → сначала правка `../rules/05-layering.md`
  (capture-политика) и этого контракта.
- Док-комментарий модуля ссылается на «Wave C §5.3.12», в спеке
  (`docs/dev-researches/ui-kit-deep-review-2026-10.md`) snackbar — §5.3.14:
  исправление комментария — задача рефакторинга.
