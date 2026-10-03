# Benchmarking 11 LLMs for Inline Autocomplete: What Actually Worked

*We needed a template autocomplete for our canvas app. We tested 11 models on OpenRouter. Here's what the numbers said — and the hybrid we shipped.*

---

## TL;DR

We benchmarked 11 LLMs (paid + free + internal) on a frozen 15-fixture eval set for template autocomplete. The lex+local-model baseline scored p@1=0.468. The best LLM hit p@1=1.000. But the path there wasn't linear — and one model with a native choice protocol surprised us.

```
baseline (lex+Laya fusion):  0.468
gemma-3-4b (4B, paid):       0.267  ❌
nemotron-nano-30b (paid):    0.467  ⚠️
phi-4 (paid):                0.667
jev-1.13 (System One):       0.733  ⚡ 0.46s latency
nemotron-super:free:         0.800  🏆 $0
glm-5.3-flash (paid):        1.000  🏆
```

---

## The setup

CanvasDesk is a visual canvas for business metrics. When a user starts typing a new node, we suggest a template from a catalog of ~19. Existing engine: lexical (BM25) + fine-tuned classifier, fused at α=0.85 → p@1=0.468.

The fine-tuned model never stably beat lexical alone. We asked: **can a general-purpose LLM via API beat the hybrid, without fine-tuning?**

### Eval set

15 choice-fixtures (frozen, sha256-pinned):
- Node context: title, canvas variables, neighbors with needs/outputs
- Catalog: 19 templates, RU/EN descriptions
- Golden: correct template_id

### Prompt

Same template for all chat-models:

```python
SYSTEM = """You are a template autocomplete engine for CanvasDesk.
Return JSON: {"choice":"<id>","confidence":0-1,"reasoning":"..."}"""

USER = f"""Context: {fixture['context']}
Catalog: {format_options(fixture['options'])}
Which template fits best?"""
```

`temperature=0`, `response_format: json_object`.

### System One protocol

One model (`typesafe/jev-1.13`) supports a native choice API:

```python
payload = {
    "model": "typesafe/jev-1.13",
    "state": {"document": fixture["context"]},
    "questions": {"main": {
        "type": "choice",
        "instructions": "Which template best fits?",
        "criteria": {o["id"]: o["desc"] for o in fixture["options"]}
    }}
}
# POST /api/alpha/decisions
# → answers.main.probabilities (native JSON, no parsing)
```

Same protocol as our existing Laya client — integration was an endpoint swap.

---

## Results

| Model | p@1 | latency | $/1000 | Notes |
|---|---|---|---|---|
| gemma-3-4b (paid) | 0.267 | 1.8s | $0.026 | Worse than baseline |
| **lex+Laya (baseline)** | **0.468** | 0.8s | $0 | |
| nemotron-nano-30b (paid) | 0.467 | 4.2s | $0.044 | = baseline |
| nemotron-3.5-lightning (paid) | 0.600 | 20.4s | $0.036 | Slow |
| phi-4 (paid) | 0.667 | 2.0s | $0.049 | |
| **jev-1.13 (System One)** | **0.733** | **0.46s** | $0.070 | Fastest, native choice |
| **nemotron-super:free** | **0.800** | 2.6s | **$0** | Best free |
| nemotron-ultra-550b:free | 0.900 | 43s | $0 | Unstable (33% errors) |
| qwen3.8-27b:free | 1.000 (7/7) | 5.8s | $0 | 53% rate-limited |
| **glm-5.3-flash (paid)** | **1.000** | 10.5s | $0.121 | Best quality |

75 requests total, $0.0163 spent.

---

## Three findings

### 1. Scale is non-linear

Models under 30B params (gemma-3-4b, nemotron-nano-30b) didn't beat the lexical baseline. Quality jumps at 120B+:

```
4B   → 0.267
30B  → 0.467
120B → 0.800
550B → 0.900
```

Counterintuitively, price doesn't track quality. The free 120B nemotron-super beat the paid 30B nemotron-nano by 71%.

### 2. Free-tier is unstable

| Free model | Result |
|---|---|
| qwen3.8-27b | 1.000 on 7/7 — but 53% rate-limited |
| gemma-4-26b | 100% geo-blocked |
| ultra-550b | 33% errors (503 overloaded) |
| nemotron-super | 15/15 clean ✅ |

Only one of four free models ran without rate limits. Free is great for prototyping; unreliable for production.

### 3. System One protocol is underappreciated

`jev-1.13` was the only model with a native choice API. Instead of:

```
LLM → text response → parse JSON → extract choice → extract confidence
```

It returns:

```json
{"answers":{"main":{"probabilities":{"id1":0.9,"id2":0.1},"confidence":0.9}}}
```

No parsing. No hallucinations on format. And **latency was 0.46s** — 5× faster than the next fastest model. For our use case (inline autocomplete where latency is critical), this was the breakthrough.

---

## What we shipped

### Hybrid for inline UX

```python
def suggest(context, catalog):
    # 1. Fast path: jev-1.13 (0.46s, p@1=0.733)
    answer = jev_client.choice(context, catalog)
    if answer.confidence >= 0.5:
        return answer.choice
    
    # 2. Quality path: glm-5.3-flash (10.5s, p@1=1.0)
    answer2 = glm_client.choice(context, catalog)
    if answer2.choice in catalog:
        return answer2.choice
    
    # 3. Safety net: lex (instant, p@1=0.404)
    return lex_rank(context, catalog)[0]
```

Cost: ~$0.08/1000 requests. Latency: 0.46s typical, 10.5s worst-case for hard fixtures.

### Pitfalls we hit

**WASM can't call APIs directly.** API key in public bundle = leak. Solution: backend proxy (desktop: sidecar, web: cloud function).

**Non-determinism breaks golden tests.** LLMs with `temperature=0` aren't 100% deterministic. Solution: disable LLM in CI, test lex only.

**Hallucinations.** LLM can return a template_id not in catalog. Validate `choice ∈ catalog`, else fallback.

---

## Cost at scale

| Scenario | Requests/month | Cost/month |
|---|---|---|
| Light user (5 edits/day) | 150 | $0.01 |
| Power user (100 edits/day) | 3,000 | $0.24 |
| Team of 10 power users | 30,000 | $2.43 |

Even a team of power users costs less than a coffee. Free-tier is $0, but with rate-limit risk.

---

## Takeaways

1. **LLM works as a ranking source.** +57–114% over baseline without fine-tuning.
2. **Scale matters.** Under 120B params, don't bother for choice-ranking.
3. **Free-tier is unstable.** Only 1 of 4 free models ran clean on 15 requests.
4. **System One protocol > chat for choice tasks.** `jev-1.13` at 0.46s with native probabilities is ideal for inline UX.
5. **Hybrid beats single-source.** jev-1.13 for speed, glm-5.3-flash for quality, lex for reliability.

Full technical report with reproducibility scripts: [dev-researches/llm-mm-source-benchmark.md](https://github.com/danku13/CanvasDesk/blob/main/docs/dev-researches/llm-mm-source-benchmark.md).

Questions? Drop them in the comments.
