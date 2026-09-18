# user-docs — пользовательская документация CanvasDesk

Краткая документация для конечных пользователей: быстрый старт, объекты
интерфейса, горячие клавиши, расчёты, FAQ.

**Основной способ чтения — встроенный просмотрщик приложения** (FR-031):
кнопка «?» → «Документация ▸». Страницы вшиты в бинарь на этапе сборки
(`include_str!`, источник — `crates/canvas-app/src/docs_ui.rs`): версия
доков = версия бинарника, работает без сети. Изменения сюда попадают через
коммит — просмотрщик подхватывает их при следующей сборке.

Публикация на **GitHub Pages** (ниже) — опциональное веб-зеркало тех же
страниц (удобно для ссылок извне и поисковиков).

| Файл | Страница |
|---|---|
| `index.md` | Главная справки, оглавление, платформы |
| `quick-start.md` | Быстрый старт за 5 минут |
| `interface.md` | Объекты интерфейса (канвас, ноды, связи, группы, миникарта, поиск, настройки) |
| `hotkeys.md` | Горячие клавиши и жесты |
| `calculations.md` | Numi-формулы, единицы, value-связи, `$in` |
| `templates.md` | Шаблоны архитектурных нод: применение, параметры и доменные типы, свой шаблон |
| `faq.md` | FAQ |
| `agent-recipe.md` | Рецепт для ИИ-агентов: сборка моделей через MCP (инструменты, порядок вызовов, лимиты, коды ошибок) |

## Публикация на GitHub Pages (опциональное веб-зеркало)

Страницы написаны с Jekyll front matter и рендерятся GitHub Pages из корня
репозитория без дополнительной настройки сборки.

1. Откройте **Settings → Pages** репозитория `danku13/CanvasDesk`.
2. **Source**: «Deploy from a branch».
3. **Branch**: `main`, папка `/ (root)`. Нажмите **Save**.
4. Через 1–2 минуты сайт будет доступен по адресу
   `https://danku13.github.io/CanvasDesk/`.

## Канонические URL для ссылок извне

После включения Pages по варианту ниже используйте эти адреса (они же —
относительные ссылки между страницами внутри папки; во встроенном
просмотрщике ссылки резолвятся по имени файла без расширения):

| Назначение | URL |
|---|---|
| Главная справки | `https://danku13.github.io/CanvasDesk/user-docs/` |
| Быстрый старт | `https://danku13.github.io/CanvasDesk/user-docs/quick-start.html` |
| Объекты интерфейса | `https://danku13.github.io/CanvasDesk/user-docs/interface.html` |
| Горячие клавиши | `https://danku13.github.io/CanvasDesk/user-docs/hotkeys.html` |
| Расчёты и поток значений | `https://danku13.github.io/CanvasDesk/user-docs/calculations.html` |
| Шаблоны нод | `https://danku13.github.io/CanvasDesk/user-docs/templates.html` |
| FAQ | `https://danku13.github.io/CanvasDesk/user-docs/faq.html` |
| Рецепт для ИИ-агентов | `https://danku13.github.io/CanvasDesk/user-docs/agent-recipe.html` |

Рекомендуемые точки подключения в приложении (реализовано, FR-031):

- кнопка **«?»** → «Документация ▸» — встроенный просмотрщик всех 7 страниц
  (правый док: скролл, внутренние ссылки);
- **F1** (оверлей хоткеев) и пункт меню «Горячие клавиши» — шпаргалка,
  полный список — страница просмотрщика `hotkeys`;
- финальная карточка онбординга (FR-028) указывает на «?» — документация и
  повтор тура;
- при первом запуске открывается ознакомительный тур (отложить —
  «Пропустить», повторить — «?» → «Пройти онбординг»).

## Альтернатива (вариант 2 — только user-docs как отдельный сайт)

Если нужно публиковать **только** `user-docs/` (без остального репо) —
включите Pages с source «GitHub Actions» и добавьте workflow:

```yaml
# .github/workflows/pages-user-docs.yml
name: pages-user-docs
on:
  push:
    branches: [main]
    paths: ["user-docs/**"]
  workflow_dispatch:
permissions:
  contents: read
  pages: write
  id-token: write
concurrency:
  group: pages
  cancel-in-progress: true
jobs:
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/configure-pages@v5
      - uses: actions/upload-pages-artifact@v3
        with:
          path: user-docs
      - id: deployment
        uses: actions/deploy-pages@v4
```

Тогда сайт будет целиком = `user-docs/`, и базовый URL станет
`https://danku13.github.io/CanvasDesk/` (без сегмента `user-docs/`) —
скорректируйте канонические ссылки в приложении.

## Правила ведения

- Новая страница: markdown с front matter (`title`, `description`), добавить
  строку в таблицы здесь и в `index.md`.
- Ссылки между страницами — относительные (`faq.html`), чтобы работали и в
  Pages, и при просмотре на GitHub.
- Содержание синхронизируется с фактами кода; глубокие внутренние детали —
  в `docs/interface-objects/`, пользовательские страницы — без ссылок на
  исходники (кроме явных случаев: SDK, BYOK, планы M7).
- Горячие клавиши держать в синхроне со списком `HOTKEYS` в
  `crates/canvas-app/src/lib.rs` (F1-оверлей) — при изменении хоткеев
  обновлять оба места.
