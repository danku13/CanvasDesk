# Technical Architect — руки на коде

## Портрет и границы роли

TA — самый «инженерный» из трёх: он сидит рядом с кодом, задаёт каркас
(модули, границы, протоколы), ревьюит PR с архитектурными последствиями и
держит качество долго после того, как SA ушёл на следующий проект. Типичный
путь в роль описан в гайде dev.to: «Making the transition from senior
developer to software architect is one of the most challenging…»
([From Senior Developer to Architect](https://dev.to/grigor_borisov_49590b5aa7/from-senior-developer-to-architect-a-complete-guide-168g))
— переход малоуправляемый, знаний не хватает на «политическую» часть роли,
а техническая перегружена.

**Ключевой продукт деятельности:** работающий каркас + правила (fitness
functions, ADR, guideline'ы), которые переживают ежедневные изменения.

## Чем измеряется успех

- Отсутствие «архитектурных» инцидентов (переписываний, узких мест);
- скорость онбординга новых инженеров в кодовую базу;
- доля PR, проходящих архитектурное ревью с первого раза.

## Топ-боли

| Ранг | Боль | Почему бьёт TA сильнее всех |
|---|---|---|
| 1 | [№1 Докум. протухает](../pain-points/01-documentation-drift.md) | именно TA замечает первым, что диаграмма врёт про прод |
| 2 | [№3 ADR не читают](../pain-points/03-adr-ignored.md) | TA пишет ADR и наблюдает, как их никто не открывает |
| 3 | [№8 ИИ-хаос](../pain-points/08-ai-governance.md) | ИИ-агенты генерируют PR, ломающие каркас; ревьюить вручную не успевают |
| 4 | [№6 Техдолг](../pain-points/06-tech-debt-economics.md) | TA платит временем команды, но не имеет денежных аргументов |
| 5 | [№9 Перегрузка](../pain-points/09-cognitive-overload.md) | держит систему в голове — единственный «человеческий компилятор» |

## Инструменты (наблюдаемое поведение)

- **Diagram-as-code**: Mermaid (встроен в GitHub), PlantUML, Structurizr DSL.
  Идея красивая, практика — так себе: «So instead of a diagram you fail to
  update over time when changing the implementation, now you've got PlantUML
  you fail to update» (обсуждение на lobste.rs, 10.01.2023; ссылку на тред
  восстановить — см. quotes-bank). Structurizr поддерживает ADR рядом с
  моделью ([blog.glen-thomas, 27.08.2025](https://blog.glen-thomas.com)),
  но остаётся нишевым; сам Simon Brown сводит вопрос к минимальному набору
  документов ([dev.to/simonbrown](https://dev.to/simonbrown/a-minimal-approach-to-software-architecture-documentation-4k6k)).
- **Fitness functions / ArchUnit-стек**: тесты на архитектурные правила —
  «automated (unit) tests for your architecture» ([InfoQ, 14.04.2025](https://www.infoq.com)),
  «shift left on governance» ([martinfowler.com, 05.09.2024](https://martinfowler.com)).
  Растущая практика, но пока для энтузиастов.
- ADR: markdown в репозитории, adr-tools, Log4brains, Backstage TechDocs.
- Ревью: GitHub/GitLab + lint-еры. AI-ревьюеры появляются, архитектуры не знают.

Разрыв: TA имеет *код-верифицируемое* намерение (тесты, ArchUnit), но не имеет
*модели*, которую можно показать человеку и посчитать в ней деньги/нагрузку.

## Каналы присутствия

- **Global:** r/softwarearchitecture, Hacker News (обсуждения C4/Structurizr),
  dev.to, lobste.rs, подкасты (Architecture Weekly).
- **RU:** Habr (профильные статьи), Telegram System Design World (подготовка
  к system design интервью — канал входа в роль), чаты @itarchitect.

## Платёжеспособность и триггеры покупки

- Лично платит неохотно (привычка к free/open-source), но **легко убеждает
  компанию**: у TA есть доверие техлида и команды. Продажа «снизу вверх».
- Триггеры: (а) инцидент/инцидент-репорт с арх. причиной; (б) рост команды
  → онбординг ломается; (в) внедрение ИИ-кодогенерации → нужна автоматическая
  проверка правил; (г) переход на микросервисы / модулит (modulith).

## Выводы для GTM

1. Для TA продукт должен жить **в их окружении**: git, PR, CI, Obsidian
   (у CanvasDesk это совместимость JSON Canvas и MCP — прямое попадание).
2. Doc-as-code — привычка, но без живого расчёта она не удерживается:
   показать «диаграмма, которая сама считает и краснеет» — сильная демо-крючка.
3. Fitness functions — тренд-союзник: позиционирование «fitness functions
   для денег и нагрузки» понятно TA без объяснений.

## Открытые вопросы

1. Какая часть вашего архитектурного каркаса проверяется машиной, а какая —
   вашей головой на ревью?
2. Когда вы в последний раз открывали чужой ADR перед изменением кода?
3. Что происходит с вашими диаграммами после релиза? Кто-то возвращает их
   в актуальное состояние?
