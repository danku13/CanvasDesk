# Боль 01: Документация и диаграммы устаревают молча

> Суть: архитектурные артефакты — единственные «факты» системы, у которых
> нет механизма, сообщающего, что они больше не соответствуют реальности.

**Матрица:** тяжесть 5/5 · распространённость 5/5 · готовность платить 2/5 ·
доказательность: высокая

## 1. Механизм боли

Код проверяется компилятором и тестами; среда — мониторингом. Документация и
диаграммы не проверяются ничем: они расходятся с реальностью тихо, без
ошибок и алертов. Чем точнее документация сегодня, тем опаснее её доверие
завтра: читатель не может отличить свежую страницу от протухшей. Для
архитектора это означает, что его главный актив — карта системы — становится
пассивом: врут и диаграммы в Miro, и runbook в Confluence, и схема микросервисов
на стене. Дрейф накапливается незаметно и обнаруживается постфактум — через
инцидент или безуспешный поиск правды.

## 2. Доказательства и цитаты

> «Documentation goes stale silently. Unlike code, there's no test suite
> that goes red when your runbook references a deprecated service»
> — Atlassian Community, февраль 2026.
> [community.atlassian.com](https://community.atlassian.com/forums/App-Central-articles/Your-Confluence-wiki-is-confidently-giving-people-wrong/ba-p/3192612)

Разбор той же статьи: «Let me guess. You have a Confluence space that's been
around for a few years. It has hundreds of pages. Some of them are great —
clear, detailed…» — и часть из них уже врёт. Формулировка «wiki is
confidently giving people wrong information right now» — центральный смысл
боли: дезинформация подана уверенно.

> «Notion wikis go stale in the same way. Google Docs accumulate drift
> identically. GitBook pages rot at the same rate» — fabric.so (вендор,
> но диагноз совпадает с независимыми источниками).
> [fabric.so/blog](https://fabric.so/blog/docs-that-write-themselves-vs-confluence)

> «So instead of a diagram you fail to update over time when changing the
> implementation, now you've got PlantUML you fail to update» — обсуждение
> diagram-as-code на lobste.rs, 10.01.2023 (прямая ссылка подлежит
> восстановлению, см. [quotes-bank](../quotes-bank.md)). Ключевое: даже
> «правильный» docs-as-code не лечит — переносит ту же дисциплину в текст.

> «After manually reverse engineering the software architecture from the
> code, they found significant architectural drift and erosion» — dev.to,
> 28.08.2024 (обратная разработка как способ узнать правду о собственной
> системе).

> «The architecture gap is the growing divergence between the intended
> design of a codebase and the actual structure that emerges as teams…» —
> SonarSource, 26.02.2026, вводит термин *architecture gap*; там же:
> «Some form of living software architecture documentation to aid teams is
> missing. That changes now» — вендор Sonar заявляет о входе в категорию.
> [sonarsource.com](https://www.sonarsource.com)

> «When architectural decisions drift or erode, the symptoms often appear
> as user-reported bugs, performance issues, or unexplained failures» —
> Multiplayer, 29.07.2025.
> [multiplayer.app/blog](https://www.multiplayer.app/blog/how-to-recover-your-architecture-after-drift-and-erosion)

Перекрёстное подтверждение из RU-сегмента: тема «нарисуй что-нибудь
архитектурное, а то мы уже код пишем» ([боль 02](02-diagrams-nobody-uses.md))
— тот же механизм: документация рождается оторванной от кода и дальше
расходится.

## 3. Как решают сегодня

- **Дисциплина ревью документации** («обнови диаграмму в этом PR») —
  работает, пока есть надзиратель; при спешке отменяется первой.
- **Diagram-as-code** (Mermaid/PlantUML/Structurizr) — версионируется с кодом,
  но не обновляется автоматически: diff кода не тянет за собой diff модели.
- **Обратная разработка** (reverse-engineering диаграмм из кода) — точная,
  но разовая и дорогая; через месяц картина снова врёт.
- **Confluence-плагины и freshness-боты** — помечают страницы старыми,
  не решая содержательного расхождения.
- **Multiplayer auto-doc, scopedocs.ai, InstantDocs, SonarSource** — новая
  волна «self-writing documentation» (детали — [competitive-analysis](../competitive-analysis.md));
  все учат документацию у кода, ни один не проверяет *смысловые* инварианты
  (нагрузка, стоимость, SLA).

## 4. Разрыв рынка

Существующие решения делают документацию **более дешёвой в обновлении**, но
не делают её **проверяемой**. Отсутствует аналог теста: свойство системы
(«максимум 200 ms на p99», «не больше $X/мес», «через кэш не больше N hop'ов»),
выраженное в документации, должно уметь «краснеть» при нарушении — в модели,
в CI или в рантайме. Категорию упоминают все, кто пишет о «living documentation»,
но рабочей реализации в массовом доступе нет (ACM, 2026: AI-based checkers
для дрейфа из-за SDK-эволюции существуют лишь как прототипы).

## 5. Следствия для продукта (гипотезы)

- **Г1.** «Живая модель» в CanvasDesk: значение на связи (`rps`, `$`, `ms`)
  — это проверяемый инвариант; если upstream-значение меняется и инвариант
  падает — модель «краснеет» визуально. (Технически почти готово: value-flow
  + Overload-детект ρ≥1.)
- **Г2.** Экспорт инвариантов в CI: модель как исполняемый тест архитектуры.
- **Г3.** Метрика «drift debt»: сколько инвариантов модели сейчас нарушено —
  превращает абстрактный дрейф в счётчик, который можно обсуждать.

## 6. Вопросы для интервью

1. Когда вы в последний раз *обнаружили*, что документация врёт? Как?
   Сколько это стоило?
2. Кто в вашей команде отвечает за актуальность архитектурных диаграмм?
3. Что бы изменилось, если бы модель «краснела» при нарушении инварианта?
4. Верите ли вы, что ваша команда будет это поддерживать? Почему?
