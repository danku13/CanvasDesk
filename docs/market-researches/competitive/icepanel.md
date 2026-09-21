# Teardown: IcePanel

> Дата разбора: 22.09.2026. Категория C (living-docs SaaS). Это конкурент №1:
> если команда сегодня выбирает «инструмент архитектурных моделей», она
> почти наверняка открывает icepanel.io первой.

## 1. Паспорт

| Поле | Значение | Источник |
|---|---|---|
| Сайт | [icepanel.io](https://icepanel.io) ✅ | — |
| Позиционирование | «Living system docs for visual people» | [Product Hunt](https://www.producthunt.com) 🌐 |
| Стадия | bootstrap, трек ~$2M ARR (июль 2025) | [блог IcePanel, 23.07.2025](https://icepanel.io/blog) ✅ (факт из [quotes-bank §10](../quotes-bank.md)) |
| Модель | C4 + flows + ADR + технологические лейблы | [docs/блог IcePanel](https://icepanel.io/blog) ✅ |
| Целевой сегмент | продуктовые инж-команды, архитекторы; SMB → mid-market | [сравнение IcePanel vs Visio, 26.05.2025](https://icepanel.io/blog) 🌐 |
| Партнёрства | публичная поддержка Simon Brown (автор C4); кейс adoption C4 с его участием | [блог IcePanel, 31.03.2025](https://icepanel.io/blog) 🌐 |

Путь к ~$2M ARR с ~$8K MRR на личных сбережениях — главный рыночный
доказатель того, что боль №01/№02 монетизируется. Но трек $2M также
значит: категория валидирована, и вслед за IcePanel в неё заходят Uxxu,
Revision, scopedocs — окно на отстройку измеряется годами.

## 2. Позиционирование и обещание

IcePanel продаёт не «диаграммы», а «цикл»: **The IcePanel Loop — Define,
Visualise, Validate, Adapt**. Ключевые фичи: строгая C4-иерархия
(landscape → system → container → component), объекты вместо стрелок
(зависимости привязаны к сущностям), fading-подсветка уровней, flows,
история версий, интеграции AWS/Azure/GCP, embed в Confluence/Notion.
Обещание покупателю: диаграммы, которые «понятно читать и не стыдно
показывать», плюс дисциплина модели, а не набора прямоугольников.

## 3. Как устроен продукт

- Редактор drag-and-drop с принудительной C4-структурой; нельзя нарисовать
  «стрелку в никуда» — связи принадлежат объектам модели.
- Views поверх модели: несколько диаграмм из одного набора сущностей.
- ADR-модуль: решения хранятся рядом с моделью (проверка боли №03
  на уровне фичи, но без связности с расчётами — см. §7).
- Экспорт/embed в Confluence, Notion, ссылки-фреймы; SSO на enterprise.
- AI-фичи дозированы (генерация черновиков), не являются ядром ставки.

## 4. Цены и упаковка (проверено 09.2026)

| Тариф | Цена | Источник |
|---|---|---|
| Free | ограничен (малый лимит объектов) | [G2-отзывы](https://www.g2.com/products/icepanel/reviews) 🌐 |
| Growth | **$40/editor/мес** при оплате годом ($50 помесячно) | [страница pricing](https://icepanel.io/pricing) ✅ + [сравнение с Visio, 26.05.2025](https://icepanel.io/blog) 🌐 |
| Enterprise | ~$25/editor/мес от 50 мест | Medium-сравнение 🌐 (вторично) |

Команда из 10 редакторов = ~$400–500/мес. Это «сид-рынок» для всех
категории C: цена нормализована на $40–50/editor — наша ценовая гипотеза
H4 ($20–40/editor) обязана учитывать, что покупатель уже привык платить
этот порядок.

## 5. Что хвалят (цитаты)

| Цитата | Источник | Статус |
|---|---|---|
| «It's user-friendly, sleek, effective. Actually, it can be used as a nice practical "demo" to convince someone that C4 …» | [No Kill Switch, 02.11.2021](https://no-kill-switch.ghost.io/system-design-with-icepanel-a-brief-and-opinionated-review/) | ✅ |
| «The tool is visually appealing, responsive, and pretty intuitive» | там же | ✅ |
| «Insanely good user experience to improve the experience of a relatively mundane but critical organizational task» | Product Hunt 🌐 | 🌐 |
| «IcePanel is primarily drag-and-drop based with a simple UI designed for collaboration» (самовердикт в сравнении с Structurizr) | [блог IcePanel, 13.11.2025](https://icepanel.io/blog) 🌐 | 🌐 |

G2: выборка малая, но 100% отзывов пятизвёздочные — типично для растущего
нишевого SaaS с «религиозной» аудиторией C4.

## 6. На что жалуются (цитаты из G2)

| Цитата | Источник | Статус |
|---|---|---|
| «Quite expensive for personal use» | [G2, раздел What do you dislike](https://www.g2.com/products/icepanel/reviews) 🌐 | 🌐 |
| «Free version is very limited (100 objects runs out very quickly)» | там же | 🌐 |
| «Limited integrations» | там же | 🌐 |
| «One of the biggest frustrations teams experience with architecture tools like IcePanel is not the quality of the individual diagrams. It is what [происходит дальше]» | [Uxxu, 06.04.2026](https://uxxu.io) ✅ | ✅ |

Перевод жалоб на язык болей: цена за редактора отсекает «личное
использование» (а именно там рождается привычка), лимит free-объектов
ломает онбординг на реальном ландшафте, а жалоба Uxxu — это наша боль №01:
даже красивая модель не обновляется сама.

## 7. Покрытие девяти болей

| Боль | Балл | Комментарий |
|---|:---:|---|
| 01 дрейф | ± | модель строже доски, но поддерживается вручную |
| 02 неиспользуемые артефакты | ± | embed в Confluence улучшает доступ, не спрос |
| 03 ADR | ± | ADR живут рядом с моделью; чтение/поиск — всё ещё на людях |
| 04 бизнес-разрыв | — | C4-язык не для бизнеса; денег в модели нет |
| 05 роль без власти | — | — |
| 06 техдолг | — | нет монетизации долга |
| 07 FinOps | — | — |
| 08 ИИ | — | AI-черновики, не guardrails |
| 09 перегрузка | ± | fading/уровни снижают шум, но это ручная дисциплина |

## 8. Разрыв и возможность CanvasDesk

IcePanel доказал: платят за **строгую модель с хорошим UX**. Он не двигается
в три места, где мы размещаемся (возможности №1, №4, №6):

1. **Исполняемая модель**: у IcePanel значения (RPS, $, latency) — статические
   лейблы. Никакой пересчёт, никакой what-if. Наш ответ — value-flow и
   Numi-выражения прямо на канвасе.
2. **Язык денег**: IcePanel не умеет NPV/бюджет — а это единственный язык,
   на котором архитектор проходит боль №04/№06.
3. **Ценовой люфт**: $50/editor/мес × вся команда — дорого для «личного
   использования» (их же G2-жалоба). Free-лимит в 100 объектов —
   агрессивный; бесплатный персональный tier с полной моделью — прямой
   контраст и воронка.

Риск: IcePanel богатее нас в 4 раза по ARR и уже добавляет AI; если они
выкачают расчётный слой раньше нас (см. риск №2 в
[competitive-analysis](../competitive-analysis.md)), преимущество сжимается.

## 9. Что мониторить

- Прайс-страница и лимиты free-тарифа (квартально).
- Появление расчётов/what-if в roadmap (ежемесячно).
- Кейсы enterprise-внедрений — как только они продают «$25 от 50 мест»
  крупным игрокам, категория консолидируется.
- Релизы и посты блога — они отлично формулируют боли (готовый
  источник цитат для [quotes-bank](../quotes-bank.md)).

## Источники

1. [icepanel.io/pricing](https://icepanel.io/pricing) ✅ — тарифы.
2. [блог IcePanel](https://icepanel.io/blog) ✅ — ARR-трек, сравнения, кейс C4.
3. [No Kill Switch, 02.11.2021](https://no-kill-switch.ghost.io/system-design-with-icepanel-a-brief-and-opinionated-review/) ✅ — независимый обзор.
4. [G2-отзывы](https://www.g2.com/products/icepanel/reviews) 🌐 — жалобы.
5. [Uxxu vs IcePanel, 06.04.2026](https://uxxu.io) ✅ — внешняя критика.
6. [Product Hunt](https://www.producthunt.com) 🌐 — позиционирование.
