# Банк цитат

> Единый реестр цитат с атрибуцией. Статус ссылки: ✅ прямая · 🌐 зеркало/вторичный · ⚠️ сниппет из поисковой выдачи (ссылку восстановить в фазе 0 — см. [research-plan](research-plan.md) §2).
> Правила отбора и этика — [00-methodology](00-methodology.md) §5, §7.

## 1. Документация и дрейф (боль 01)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «Documentation goes stale silently. Unlike code, there's no test suite that goes red when your runbook references a deprecated service» | Atlassian Community — [ссылка](https://community.atlassian.com/forums/App-Central-articles/Your-Confluence-wiki-is-confidently-giving-people-wrong/ba-p/3192612) | 02.2026 | ✅ |
| «Your Confluence wiki is confidently giving people wrong information right now» (заголовок) | там же | 02.2026 | ✅ |
| «Notion wikis go stale in the same way. Google Docs accumulate drift identically. GitBook pages rot at the same rate» | Fabric — [ссылка](https://fabric.so/blog/docs-that-write-themselves-vs-confluence) | 2026 | ✅ |
| «So instead of a diagram you fail to update over time when changing the implementation, now you've got PlantUML you fail to update» | lobste.rs, тред «Architecture diagrams should be code» (подтверждён, прямой /s/-путь закрыт сетью; вторичный: [lobste.rs/~sidmitra](https://lobste.rs/~sidmitra)) | 10.01.2023 | 🌐 |
| «After manually reverse engineering the software architecture from the code, they found significant architectural drift and erosion» | dev.to — «[Part 2] Architectural Drift in Real Life Systems» (подтверждена, прямой путь не восстановлен) | 28.08.2024 | 🌐 |
| «The architecture gap is the growing divergence between the intended design of a codebase and the actual structure that emerges…» | SonarSource — [блог](https://www.sonarsource.com) | 26.02.2026 | ✅ |
| «Some form of living software architecture documentation to aid teams is missing. That changes now» | SonarSource — там же | 26.02.2026 | ✅ |
| «When architectural decisions drift or erode, the symptoms often appear as user-reported bugs, performance issues, or unexplained failures» | Multiplayer — [блог](https://www.multiplayer.app/blog/how-to-recover-your-architecture-after-drift-and-erosion) | 29.07.2025 | ✅ |

## 2. Артефакты «непонятно для кого» (боль 02)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «Эти архитекторы делают непонятно для кого», «Я тут в Miro накидал», «Нарисуй там что-нибудь архитектурное, а то мы уже код пишем» — знакомо? | Антон Ущаповский — [Habr 1009402](https://habr.com/ru/articles/1009402/) | 2026 | ✅ |
| «C4 diagrams are only useful when teams can actually navigate and maintain them. Otherwise, architecture becomes another source of confusion» | Uxxu — [сайт](https://uxxu.io) | 06.04.2026 | ✅ |
| «The question of "how much documentation should we write?" is popping up a lot recently» | Simon Brown — [dev.to](https://dev.to/simonbrown/a-minimal-approach-to-software-architecture-documentation-4k6k) | 2025 | ✅ |
| «…ineffective knowledge [management]» (в топ-5 барьеров EA) | Hillmann, 2025 — [scitepress](https://www.scitepress.org) | 2025 | ✅ |

## 3. ADR (боль 03)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «I'm sure this will be a controversial post. But, wondering how the architects here feel about ADRs…» | Reddit — тред [«Are ADRs a Waste of Time?», r/EnterpriseArchitect](https://www.reddit.com/r/EnterpriseArchitect/comments/1pmszhs/are_adrs_a_waste_of_time/) (сабреддит исправлен по факту) | 2026 | ✅ |
| «The reason is almost always that nobody decided when an ADR is required, who reviews it, or where it lives — engineers do not look» | Reddit (вероятно, тред 1pmszhs выше — проверить вручную) | 08.05.2026 | ⚠️ |
| «Often these decisions are made and changed blindly with little record of why that decision was made…» | Reddit | 26.06.2023 | ⚠️ |
| «A 2025 study on ADR effectiveness found that teams spending 20–30% of coordination time on architectural alignment often lack sustainable…» | первоисточник не найден повторным поиском (выдача забита юридическим ADR) — кандидат на удаление | 2025 | ⚠️ |
| «10 Frequent Decision Recording Issues: The context is underspecified. Evaluation criteria are not made explicit…» | ozimmer — [«Ten Common Mistakes in Architectural Decision Records»](https://ozimmer.ch/practices/2026/09/12/ADRMistakes.html) | 12.09.2026 | ✅ |
| «Structurizr supports managing ADRs alongside your architecture model, making it easy to keep decisions visible and version-controlled» | blog.glen-thomas — [ссылка](https://blog.glen-thomas.com) | 27.08.2025 | ✅ |

## 4. Бизнес и стейкхолдеры (боль 04)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «One of the biggest challenges in any project is managing multiple stakeholders with different expectations» | Angela Wick — [LinkedIn-пост](https://www.linkedin.com/posts/angelawickcbap_stakeholder-alignment-is-not-about-getting-activity-7416237483484561409-870e) | ~2026 | ✅ |
| «Effective architecture grows through clarity of purpose, stakeholder alignment, and disciplined governance — not just volume or reach» | Gareth Guest — [LinkedIn-пост](https://www.linkedin.com/posts/garethguest_architecture-enterprisearchitecture-solutionarchitecture-activity-7339630365705207808-Q2l8) | 2025 | ✅ |
| «Lack of business buy-in… Fragmented tools and data… Disconnection from business transformation initiatives» | Blue Dolphin — [ссылка](https://bluedolphin.io) | 06.05.2025 | ✅ |
| «…communication problems, limited top management support…» | Hillmann — [scitepress](https://www.scitepress.org) | 2025 | ✅ |
| «Архитектура в ИТ — это не "нарисовать диаграмму" и не "выбрать стек". Это работа со сложностью…» | Филипп Дельгядо — [Habr 1015988](https://habr.com/ru/articles/1015988/) | 2026 | ✅ |
| «…как нам заказчик объяснил, так мы и сделали» — слабый аргумент; причина — «отсутствие экспертизы в области архитектуры бизнеса» | habr-all.livejournal.com | 15.10.2025 | ✅ |

## 5. Роль без власти (боль 05)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «…с ролью архитектора решений история очень интересная, так как этим специалистам часто…» (FAQ о неоднозначности роли) | Росбанк — [Habr 803275](https://habr.com/ru/companies/rosbank/articles/803275/) | 2025 | ✅ |
| «Олимп айтишников… Ореол загадочности и элиты вокруг этой профессии не даёт покоя амбициям юных» | [Habr 583726](https://habr.com/ru/articles/583726/) | 2021 | ✅ |
| «Мифы об ИТ-архитектуре, из-за которых ваш проект стоит дороже» | Александр Виноградов (Ви.Tech) — [Habr 932640](https://habr.com/ru/articles/932640/) | 2025 | ✅ |
| «You're the architect anyway» (афоризм ловушки ответственности без полномочий) | фольклор Reddit/HN без единого первоисточника — использовать только как мем-референс | — | ⚠️ |
| «Solutions Architect has a 25% AI replacement risk (Low Risk)… Cross-organizational architecture needs human judgment» | willitreplace.me | 2026 | ✅ |

## 6. Техдолг (боль 06)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «42% of every developers' working week is spent dealing with technical debt (13.5 hrs) and bad code (3.8 hrs)» | Stripe Developer Coefficient — 🌐 [пересказ tiny.cloud](https://www.tiny.cloud); оригинал: [PDF Stripe](https://stripe.com/files/reports/the-developer-coefficient.pdf) | 2018 | 🌐 |
| «Technical debt amounts to up to 40 percent of their entire technology estate» | McKinsey 2022 — 🌐 [пересказ vfunction](https://vfunction.com) | 2022 | 🌐 |
| «Developers waste, on average, 23% of their development time due to TD» | getdx (обзор исследований) | 2025 | ✅ |
| «69% of developers lose 8 hours or more per week to inefficiencies… 33% of total developer time goes to tech debt and maintenance» | ShiftMag — [ссылка](https://shiftmag.dev) | 14.08.2024 | ✅ |
| «Nearly 70% of organizations view technical debt as having a high level of impact on their ability to innovate» (+ «organizations today spend an average of 30% of their IT budgets») | Protiviti — [«Technical Debt and Innovation – the CFO's Perspective»](https://www.protiviti.com) (путь домен-уровня; вторичный: [CFO Dive](https://www.cfodive.com)) | — | 🌐 |

## 7. Облако и FinOps (боль 07)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «…many FinOps practitioners are encountering difficulty in getting engineers to act on their cost…» | FinOps Foundation — [finops.org](https://www.finops.org) | 2025–2026 | ✅ |
| «…developers were often far from culpability: Cloud operations team…» | Harness — [harness.io](https://www.harness.io) | — | ✅ |
| «Modern FinOps cost optimization focuses on visibility, ownership, and value» | Zylo — [zylo.com](https://zylo.com) | 16.04.2026 | ✅ |

## 8. ИИ и governance (боль 08)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «We built Architect MCP to close the feedback loop at code generation time; Results: 80% pattern compliance vs 30–40% with documentation alone» | dev.to — [«AI Keeps Breaking Your Architectural Patterns. Documentation Won't Fix It»](https://dev.to/vuong_ngo/ai-keeps-breaking-your-architectural-patterns-documentation-wont-fix-it-4dgj) | 12.10.2025 | ✅ |
| «AI-based architecture checkers have been prototyped for integration into CI/CD pipelines to detect architectural drift caused by SDK evolution» | Bucaioni et al., ACM | 2026 | ✅ |
| «…embrace AI» в Leadership Vision for EA | Gartner — 🌐 [пересказ timoelliott](https://timoelliott.com) | 07.02.2025 | 🌐 |
| «84% of architects see AI as augmenting their work, not replacing it» | Monograph | 23.12.2025 | ✅ |
| «90% of Enterprise Architecture Deliverables Have to Change by 2029» (аналитик Rimma Gurevich) | Gartner — 🌐 [пересказ Nile](https://nilesecure.com) | 05.05.2026 | 🌐 |

## 9. Карьера и перегрузка (боль 09)

| Цитата | Автор / источник | Дата | Статус |
|---|---|---|---|
| «Making the transition from senior developer to software architect is one of the most challenging…» | Grigor Borisov — [dev.to](https://dev.to/grigor_borisov_49590b5aa7/from-senior-developer-to-architect-a-complete-guide-168g) | — | ✅ |
| «…there is often not enough clarity» (про роль SA) | Level Up Coding — «7 Challenges Encountered by Solutions Architects and How to Overcome Them» (подтверждена, прямой medium-путь не восстановлен) | 06.03.2024 | 🌐 |
| «Все последние SD интервью, включая собеседование на позицию архитектора мною пройдены» | System Design World, Telegram (канал не индексируется; подтверждается только выгрузкой) | — | ⚠️ |
| «…существует много типов роли архитектора: технический архитектор, архитектор приложений, архитектор решений…» | Habr — «Будни архитектора решений. Или кто он такой и чем занимается каждый день?» (заголовок и дата подтверждены, ID статьи не восстановлен) | 28.02.2023 | ⚠️ |

## 10. Рынок, зарплаты, цены конкурентов

| Факт | Источник | Дата | Статус |
|---|---|---|---|
| SA в РФ: 300–800K ₽/мес, медиана 465K ₽ | h.careers | 30.06.2026 | ✅ |
| Вилка Solution Architect 282–533K ₽ | quick-offer.ru | 06.05.2026 | ✅ |
| Медиана SA (глобал): $215 000 (1 317 профилей) | levels.fyi | 2026 | ✅ |
| EA-платформы: $5–15K / $50–200K / $250–750K в год | Ardoq (pricing breakdown) | 15.08.2025 | ✅ |
| Essential EA-стек: $21 999/год плоско | enterprise-architecture.org | 2026 | ✅ |
| IcePanel: $50/editor/мес; $40 годовой; enterprise $25 (от 50 мест) | [icepanel.io/pricing](https://icepanel.io/pricing) ✅ + Medium (IcePanel vs LucidChart) 🌐 | 2025–2026 | ✅ |
| IcePanel: от ~$8K MRR на личных сбережениях к треку $2M | блог IcePanel | 23.07.2025 | ✅ |
| Gartner: Ardoq 4.8★ (233 отзыва), LeanIX 4.7★ (479) | Gartner Peer Insights | 2026 | ✅ |

## 11. Цитаты конкурентов (teardown, 09.2026)

Материал для [competitive/](competitive/README.md) — как рынок сам
формулирует наши боли:

| Цитата / факт | Источник | Дата | Статус |
|---|---|---|---|
| «Quite expensive for personal use. Free version is very limited (100 objects runs out very quickly). Limited integrations» | G2 — [IcePanel, dislike-раздел](https://www.g2.com/products/icepanel/reviews) | 2026 | 🌐 |
| «One of the biggest frustrations teams experience with architecture tools like IcePanel is not the quality of the individual diagrams…» | Uxxu — [сравнение](https://uxxu.io) | 06.04.2026 | ✅ |
| «IcePanel is primarily drag-and-drop based with a simple UI designed for collaboration» | [блог IcePanel](https://icepanel.io/blog) — сравнение со Structurizr | 13.11.2025 | 🌐 |
| «Software architecture diagrams maintained "as code" have a higher initial learning curve than UI-driven tools, but offer significant long-term advantages» | [docs.structurizr.com](https://docs.structurizr.com) — «Why "as code"?» | — | ✅ |
| «Using the Structurizr server via the prebuilt binaries requires a license. Pricing applies to each Structurizr server installation» | [Patreon](https://www.patreon.com) — «Introducing Structurizr vNext» | 02.01.2026 | ✅ |
| On-premises репозиторий Structurizr архивирован («will not receive any further updates — migrate to server») | [GitHub](https://github.com) | 28.03.2026 | ✅ |
| «Multiplayer has raised a total funding of $3M over 1 round… Seed… 07.08.2023» | [Tracxn](https://tracxn.com) + [VentureBeat](https://venturebeat.com) | 02.08.2026 | 🌐 |
| «Architecture diagrams get outdated because they are disconnected from code» | [trytentra](https://trytentra.com) | 08.03.2026 | 🌐 |
| «Architecture diagrams do not go stale because engineers stop caring. They go stale because the tools that produce them exist outside the [системы правды]» | dev.to — [erajasekar](https://dev.to/erajasekar/the-real-reason-architecture-diagrams-go-stale-35ok) | 31.03.2026 | ✅ |
| «Most architecture diagrams are outdated the moment you finish drawing them. The drag-and-drop approach guarantees drift» | [idontlikeai.dev](https://www.idontlikeai.dev) | 2026 | 🌐 |
| Miro: Starter $8, Business $20 (годом) / $25 (помесячно), Enterprise custom | [help.miro.com](https://help.miro.com) ✅ + [figr](https://figr.design) 🌐 + [spendhound](https://www.spendhound.com) 🌐 | 03–04.2026 | ✅ |
| Ardoq: «modular, app-based pricing… based on the number of applications managed» | [Gartner PI](https://www.gartner.com) | 2026 | 🌐 |
| «Some reviewers call out a steep learning curve» (Ardoq) | [rfp.wiki](https://www.rfp.wiki) — Ardoq vs ADOIT | 2026 | 🌐 |
| Ardoq: 95% willingness to recommend, support 4.9/5 (Peer Insights «Customer First» 2026) | [ardoq.com](https://www.ardoq.com) | 15.04.2026 | ✅ |
| Multiplayer: MCP-сервер «to provide rich engineering context to AI coding agents» | [zenml.io](https://www.zenml.io) | 2026 | 🌐 |

## Задолженность по ссылкам (фаза 0) — статус на 22.09.2026

Прогресс: 15 позиций исходно, **8 гашено** (5 → ✅, 3 → 🌐), 7 осталось.

Восстановлено прямыми ссылками:
- ✅ Reddit «Are ADRs a Waste of Time?» (r/EnterpriseArchitect — сабреддит
  в исходной строке был указан неверно, исправлено);
- ✅ ozimmer «Ten Common Mistakes in ADR»;
- ✅ LinkedIn-посты Angela Wick и Gareth Guest (просмотр может требовать
  входа в LinkedIn — статус ✅ означает прямую ссылку, не доступность анонимно);
- ✅ dev.to «AI Keeps Breaking Your Architectural Patterns»;
- ✅ IcePanel pricing (прямой прайс вместо Medium-пересказа).

Повышено до 🌐 (подтверждены существование/дата/название, прямой путь
закрыт сетью или выдачей): lobste-тред, dev.to «Architectural Drift in
Real Life Systems», Protiviti CFO-статья, Level Up Coding «7 Challenges».

Осталось (причины):
- Reddit-цитаты §3.2/§3.3 — login-wall Reddit; вероятно §3.2 из треда 1pmszhs;
- «You're the architect anyway» — фольклор без первоисточника (оставлено
  как мем-референс, из публичных материалов исключать);
- ADR-статья «20–30% coordination time» — повторный поиск не подтвердил
  существование исследования; **кандидат на удаление**;
- Telegram «SD интервью» — каналы не индексируются;
- Habr «Будни архитектора решений» (28.02.2023) — заголовок/дата
  подтверждены, ID статьи не восстановлен (Habr-поиск JS-рендер).

До восстановления оставшиеся ⚠️ использовать только для внутренней работы
(см. [00-methodology](00-methodology.md) §5, §7).
