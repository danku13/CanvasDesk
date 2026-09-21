# Teardown: Structurizr

> Дата разбора: 22.09.2026. Категория B (diagram-as-code). Интеллектуальный
> референс всей C4-индустрии: любой конкурент в питче сравнивает себя
> со Structurizr. Одновременно — живой урок того, как «правильная идея»
> без продуктовой машины застревает в нише.

## 1. Паспорт

| Поле | Значение | Источник |
|---|---|---|
| Сайт | [structurizr.com](https://structurizr.com) ✅ | — |
| Автор | Simon Brown, автор C4-модели | [docs.structurizr.com](https://docs.structurizr.com) ✅ |
| Позиционирование | «C4 models as code»: DSL → один модель → много диаграмм | [docs.structurizr.com](https://docs.structurizr.com) ✅ |
| Состав | DSL (текст), cloud-сервис, on-premises, экспорт PlantUML/Mermaid | docs ✅ |
| Стадия (ключевое!) | on-premises **архивирован 28.03.2026**, миграция на лицензируемый «server» (vNext) | [GitHub, 28.03.2026](https://github.com) ✅ + [Patreon, 02.01.2026](https://www.patreon.com) ✅ |
| Целевой сегмент | инженеры, любящие код; архитектурные пуристы | docs ✅ |

## 2. Позиционирование и обещание

«Модель важнее картинки» — Structurizr доказал эту идею раньше всех:
единый DSL-модель порождает согласованную систему представлений, и
изменение сущности обновляет все диаграммы. Вендор сам честно описывает
цену входа:

> «Software architecture diagrams maintained "as code" have a higher
> initial learning curve than UI-driven tools, but offer significant
> long-term advantages» — [docs.structurizr.com, Why "as code"?](https://docs.structurizr.com) ✅

Это редкий случай, когда vendor прямо признаёт барьер — и он же
объясняет, почему категория B не съела Miro: стейкхолдеры DSL не читают.

## 3. Как устроен продукт

- **Structurizr DSL**: текстовое описание модели (workspace → model → views).
- **Cloud**: публикует диаграммы, ревизии, embed в Confluence.
- **On-premises → server (vNext)**: standalone-установка Jakarta EE/Spring
  объявлена устаревшей; развитие продолжится в лицензируемом server-дистрибутиве.
- ADR-поддержка: решения хранятся рядом с моделью ([glen-thomas, 27.08.2025](https://blog.glen-thomas.com) ✅).
- Экспорт в PlantUML/Mermaid для CI-пайплайнов.
- Развитие живо: DSL v4.0.0 готовит relationship archetypes
  ([bsky Simon Brown](https://bsky.app) 🌐; интервью [Nerd Noir, 17.06.2026](https://newsletter.nerdnoir.com) 🌐).
  Слухи о «смерти Structurizr» преждевременны, но модель развития сместилась
  к Patreon-подписке и лицензиям — тонкий ручеёк вместо продуктовой компании.

## 4. Цены и упаковка (проверено 09.2026)

| Позиция | Цена | Источник |
|---|---|---|
| DSL / open source-клиент | $0 | docs ✅ |
| On-premises (legacy) | подписка; «perpetual usage» сохраняется за платившими, обновлений не будет | [FAQ cloud](https://structurizr.com) ✅ + [GitHub-архив](https://github.com) ✅ |
| **Server (vNext)** | «requires a license. Pricing applies to each Structurizr server installation» — конкретные цифры не опубликованы | [Patreon, 02.01.2026](https://www.patreon.com) ✅ |
| Cloud | публичные тарифы в индексах не зафиксированы | structurizr.com ✅ (FAQ) |

Вывод по прайсу: деньги монетизируются через **установку сервера**, а не
через место работы команды. Для mid-market это операционная головная боль,
а не SaaS-подписка.

## 5. Что хвалят (цитаты)

| Цитата | Источник | Статус |
|---|---|---|
| «Structurizr offers a code-based approach for creating and managing C4 model architecture diagrams, addressing common challenges…» | [mydeveloperplanet, 20.03.2024](https://mydeveloperplanet.com) 🌐 | 🌐 |
| «The C4 model works exceptionally well with a tool called Structurizr, which was also developed by Simon Brown» | [schibsted-vend, 09.01.2025](https://schibsted-vend.pl) 🌐 | 🌐 |
| Сообщество ценило именно модельность (обзоры 2024–2026) | [competitive-analysis, категория B](../competitive-analysis.md) | — |

## 6. На что жалуются

| Симптом | Доказательство | Статус |
|---|---|---|
| Барьер DSL отсекает стейкхолдеров | признание вендора в «higher initial learning curve» (§2) | ✅ |
| Страх заброшенности — теперь факт: on-prem-репозиторий «will not receive any further updates» | [GitHub, 28.03.2026](https://github.com) ✅ | ✅ |
| «So instead of a diagram you fail to update… now you've got PlantUML (or Structurizr DSL) you fail to update» — дисциплина всё равно на людях | [lobste.rs, 10.01.2023](https://lobste.rs) 🌐 (тред «Architecture diagrams should be code», подтверждён; прямой /s/-путь закрыт сетью на момент разбора) | 🌐 |
| «C4 diagrams are only useful when teams can actually navigate and maintain them. Otherwise, architecture becomes another source of confusion» | [Uxxu, 06.04.2026](https://uxxu.io) ✅ — конкурент бьёт по слабому месту референса | ✅ |

## 7. Покрытие девяти болей

| Боль | Балл | Комментарий |
|---|:---:|---|
| 01 дрейф | ± | модель обновляется централизованно, но только если кто-то пишет DSL |
| 02 артефакты | — | стейкхолдер без обучения не читает DSL-выход |
| 03 ADR | ± | ADR рядом с моделью — лучшая практика категории |
| 04 бизнес-разрыв | — | — |
| 05 роль без власти | — | — |
| 06 техдолг | — | — |
| 07 FinOps | — | — |
| 08 ИИ | — | DSL машинно-читаем, но MCP-интеграции нет |
| 09 перегрузка | — | «higher initial learning curve» = налог на каждого нового участника |

## 8. Разрыв и возможность CanvasDesk

Structurizr — доказательство верхней планки ценности: **одна модель, много
представлений**. Наша отстройка тройная:

1. **Визуальный вход вместо DSL**: мы сохраняем модельность, но убираем
   «higher initial learning curve» — значения вводятся прямо в узлы канваса.
   Это снимает боль №09 для 80% команды, которые DSL не осилят.
2. **Исполнение вместо описания**: DSL описывает структуру; наш граф
   считает очередь и деньги (возможности №1, №4, №6). «Почему так?» —
   это вопрос не к диаграмме, а к расчёту.
3. **Продуктовая машина против авторской подписки**: переход on-prem →
   лицензируемый server (архивация 28.03.2026) — окно для команд,
   которым нужен self-hosted без enterprise-контракта; и урок для нас:
   SaaS-прайс с публичным self-host планом снимает страх «инструмент умрёт».

Риск: авторитет Simon Brown таков, что любая C4-фича Structurizr
де-факто становится стандартом; наш формат модели обязан уметь
экспортироваться в DSL/JSON — иначе мы «не совместимы с C4-миром».

## 9. Что мониторить

- Публикация прайса server/vNext и даты EOL on-prem.
- DSL v4.0.0: relationship archetypes — не конфликтует ли с нашей моделью рёбер.
- Активность сообщества (Reddit/HN) вокруг миграции — источник мигрирующих
  пользователей.

## Источники

1. [docs.structurizr.com — Why "as code"?](https://docs.structurizr.com) ✅
2. [GitHub: Structurizr on-premises archived — 28.03.2026](https://github.com) ✅
3. [Patreon: Introducing Structurizr vNext — 02.01.2026](https://www.patreon.com) ✅
4. [FAQ cloud: on-premises subscriptions / perpetual usage](https://structurizr.com) ✅
5. [mydeveloperplanet, 20.03.2024](https://mydeveloperplanet.com) 🌐
6. [Nerd Noir, 17.06.2026](https://newsletter.nerdnoir.com) 🌐; [bsky Simon Brown](https://bsky.app) 🌐
