# Contributing to CanvasDesk · Как внести вклад в CanvasDesk

> English below · Русская версия ниже

---

## Русская версия

Спасибо за интерес к проекту! Приветствуется любой вклад: багфиксы, тесты, документация, шаблоны моделей, виджеты, примеры, идеи.

### 1. CLA — важно прочитать

Отправляя Pull Request или иной вклад, вы **автоматически принимаете [CLA](CLA.md)**:

- авторство вашего кода **остаётся за вами**;
- вы предоставляете владельцу проекта право использовать и лицензировать ваш вклад в будущем **на любых условиях** (включая коммерческие лицензии).

Это позволяет CanvasDesk оставаться открытым (GNU AGPLv3) и одновременно развивать коммерческие редакции. Если условия CLA вам не подходят — просто не отправляйте вклад.

### 2. Как отправить изменения

1. Создайте форк и ветку с понятным именем (`feature/<имя>` или по индексу CR/FR: `feature/fr-061-...`).
2. Убедитесь, что локальные гейты зелёные:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets`
   - `cargo test --workspace`
   - для web-волны (wasm): `scripts/wasm_gate.sh`
3. Откройте Pull Request по [шаблону](.github/PULL_REQUEST_TEMPLATE.md) и **отметьте чекбокс CLA** — без него workflow `cla-check` пометит PR как непринятый и merge будет заблокирован.
4. Крупные изменения (новая функциональность, изменение архитектуры) предварительно обсудите в issue — проект ведёт [индекс CR/FR](docs/change-requests/index-cr-fr.md).

### 3. Стиль

- Коммиты: `feat(scope): описание`, `fix(scope): описание`, `docs: описание` — проект ведёт журнал изменений по FR/CR-номерам (см. git log и [docs/change-requests/](docs/change-requests/)).
- Внутренние правила разработки — [AGENTS.md](AGENTS.md); архитектурные решения — [docs/adr/](docs/adr/README.md).
- Документация пользователя — [user-docs/](user-docs/); при добавлении фич обновите её при необходимости.

### 4. Лицензия вклада

Внося изменения, вы соглашаетесь, что они войдут в Проект под **GNU AGPLv3**; дополнительные права владельцу передаются согласно [CLA](CLA.md). Новые зависимости согласовывайте с [deny.toml](deny.toml) и [about.toml](about.toml) (реестр сторонних лицензий — [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)).

---

## English version

Thanks for your interest in the project! Any contribution is welcome: bugfixes, tests, documentation, model templates, widgets, examples, ideas.

### 1. CLA — please read

By opening a pull request or submitting any contribution, you **automatically accept the [CLA](CLA.md)**:

- **authorship of your code stays with you**;
- you grant the project owner the right to use and license your contribution in the future **on any terms** (including commercial licenses).

This keeps CanvasDesk open (GNU AGPLv3) while allowing commercial editions to evolve. If you disagree with the CLA terms — simply do not submit a contribution.

### 2. How to submit changes

1. Fork the repository and create a clearly named branch (`feature/<name>` or via the CR/FR index: `feature/fr-061-...`).
2. Make sure the local gates are green:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets`
   - `cargo test --workspace`
   - for the web (wasm) track: `scripts/wasm_gate.sh`
3. Open a pull request using the [template](.github/PULL_REQUEST_TEMPLATE.md) and **tick the CLA checkbox** — otherwise the `cla-check` workflow will flag the PR as unaccepted and the merge will be blocked.
4. Discuss large changes (new features, architecture changes) in an issue first — the project maintains a [CR/FR index](docs/change-requests/index-cr-fr.md).

### 3. Style

- Commits: `feat(scope): description`, `fix(scope): description`, `docs: description` — the project tracks changes by FR/CR numbers (see git log and [docs/change-requests/](docs/change-requests/)).
- Internal development rules — [AGENTS.md](AGENTS.md); architectural decisions — [docs/adr/](docs/adr/README.md).
- User documentation lives in [user-docs/](user-docs/); update it when adding features.

### 4. Contribution licensing

By submitting changes you agree that they will become part of the Project under **GNU AGPLv3**; additional rights are granted to the owner per the [CLA](CLA.md). Coordinate new dependencies with [deny.toml](deny.toml) and [about.toml](about.toml) (third-party license registry — [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)).
