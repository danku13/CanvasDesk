# Golden-фикстуры canvas-suggest

Происхождение: PoC Laya (архив `laya-poc-2026-09-27.tar.gz`, harness-коммит
`e6e666e`, тег `baseline-frozen`) + активы репозитория CanvasDesk. Все файлы
детерминированы; изменение любого из них требует явного решения (гейты
`tests/golden_*.rs` привязаны к замороженным числам отчётов волн 1–3,
`docs/dev-researches/laya-poc-report.md`).

| Файл | Что | Источник | Размерность |
|---|---|---|---|
| `evalset/source_a.jsonl` | фикстуры источника А (карвинг 11 схем + demo-numi) | архив, `harness/evalset/` | 175 |
| `evalset/source_b.jsonl` | синтетика по user stories (источник В) | архив, `harness/evalset/` | 51 |
| `laya_mm.json` | замороженные пробы laya-multilingual (probs по каждой choice-фикстуре) | архив, `harness/report/runs/` | 226 |
| `catalog_names.json` | имена шаблонов каталога (key, name_ru, name_en) — для lex v2 | архив, `harness/corpus/templates/` (62 манифеста) | 62 |
| `domain_schemes.jsonl` | схемы канвасов для домен-детектора | репо, `assets/canvas-schemes/` | 14 схем (3 framework), 36 карвингов |
| `context_pairs.jsonl` | пары вход→контекст для byte-теста сериализатора | сняты с эталонного `serialize.py` на схемах репо | 8 |

Контроль целостности (sha256, первые 16 символов):

- `evalset/source_a.jsonl` — `e0701526b9f5194f` (совпадает с
  `harness/evalset/manifest.json` волны 1);
- `evalset/source_b.jsonl` — `5be9428ce2b53370` (там же).

## Регенерация

```bash
# архив PoC распакован, путь до harness в POC_HARNESS
python3 scripts/gen_suggest_fixtures.py   # вне репо; источник — см. FR-079
```

Генератор живёт вне репозитория (зависит от распакованного архива PoC);
при изменении формата фикстур перегенерировать и сверить числа гейтов
с отчётами волн 1–3 (lex 0.319/0.413/0.381; fusion 0.468; gate 37/39).

## Почему пробы, а не живой инференс

Гибрид полностью детерминированно тестируем офлайн по замороженным пробам
волны 1: sidecar в CI не нужен, GPU не нужен, сеть не нужна (FR-079
§Verification). Живой инференс — это S2-smoke (feature `l1-laya`), не CI.
