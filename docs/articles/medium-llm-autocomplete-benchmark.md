# We benchmarked 11 LLMs for inline autocomplete. One unexpected model won.

*How a canvas-based business-metrics app replaced its fine-tuned classifier with a general-purpose LLM — and what the numbers actually say.*

---

## The bottom line

We ran 11 LLMs (5 paid, 5 free, 1 internal) on a frozen eval set for template autocomplete. The results overturned our assumptions:

- **LLM beats our lex+fine-tuned hybrid by 57–114%** (p@1: 0.468 → 0.800–1.000)
- **Scale matters non-linearly**: models under 30B params don't beat the lex baseline; 120B+ jumps to 0.800–1.000
- **Price/quality doesn't correlate**: a free 120B model beat a paid 30B model by 71%
- **One model (jev-1.13) with native System One protocol hit 0.46s latency** — 5× faster than the next fastest, with a drop-in integration path

This post covers what we measured, what surprised us, and the hybrid we settled on.

---

## The problem

CanvasDesk is a visual canvas for business metrics — think Miro for unit economics. Users place metric nodes (CAC, LTV, margin), connect them with value-flow edges. When a user starts typing a new node, the app suggests a matching template from a catalog of ~20.

Our existing suggest engine is a hybrid:

- **Lexical** (BM25 + char3 + synonyms) — fast, local, deterministic, p@1 = 0.404
- **Fine-tuned classifier** (Laya, local sidecar) — slower, "understands" context, p@1 = 0.383 alone
- **Fusion**: `α·lex + (1−α)·mm`, α = 0.85 → **p@1 = 0.468**

The fine-tuned model never stably beat the lexical baseline (confidence interval crossed zero). The question: **can a general-purpose LLM via API beat the hybrid — without any fine-tuning?**

---

## What we measured

### The eval set

15 choice-fixtures (subset of the 47-fixture test split), frozen and sha256-pinned:
- Node context: title, canvas variables, neighbors (up/down) with their needs/outputs
- Catalog: 19 templates with RU/EN descriptions
- Golden: the correct template_id
- Scenarios: C3 (title present, easier) and C4 (title empty, harder)

### The prompt

One template for all chat-models (no few-shot, no chain-of-thought):

```
SYSTEM: You are a template autocomplete engine for CanvasDesk.
        Return JSON: {"choice":"<id>","confidence":0-1,"reasoning":"..."}

USER:   Context: {node context}
        Catalog: {19 options}
        Which template fits best?
```

Parameters: `temperature=0`, `response_format: json_object`.

### System One protocol

One model (`typesafe/jev-1.13`) supports a native choice protocol — the same one our existing Laya client uses. Request:
```json
{"model":"jev-1.13","state":{"document":"..."},
 "questions":{"main":{"type":"choice","criteria":{"id":"desc",...}}}}
```
Response: `answers.main.probabilities` — native JSON, no text parsing.

### Models tested

5 paid (gemma-3-4b, nemotron-nano-30b, nemotron-3.5-lightning, phi-4, glm-5.3-flash, jev-1.13), 5 free (nemotron-super, nemotron-ultra-550b, qwen3.8-27b, gemma-4-26b, inkling), 1 internal (GLM API). Total 75 requests, $0.0163 spent.

---

## Results

### Full table

| Model | p@1 | avg latency | $/1000 | Status |
|---|---|---|---|---|
| gemma-3-4b (4B, paid) | 0.267 | 1.8s | $0.026 | ❌ worse than baseline |
| **lex+Laya (baseline)** | **0.468** | 0.8s | $0 | — |
| nemotron-nano-30b (paid) | 0.467 | 4.2s | $0.044 | ⚠️ = baseline |
| nemotron-3.5-lightning (paid) | 0.600 | 20.4s | $0.036 | ⚠️ slow |
| phi-4 (paid) | 0.667 | 2.0s | $0.049 | ✓ |
| **jev-1.13 (System One)** | **0.733** | **0.46s** | $0.070 | ⚡ unique |
| **nemotron-super:free** | **0.800** | 2.6s | **$0** | 🏆 free |
| nemotron-ultra-550b:free | 0.900 | 43s | $0 | ⚠️ unstable |
| qwen3.8-27b:free | 1.000 (7/7) | 5.8s | $0 | ⚠️ rate-limited |
| **glm-5.3-flash (paid)** | **1.000** | 10.5s | $0.121 | 🏆 ideal |

### Three findings that changed our approach

**1. Scale is non-linear.** Models under 30B params (gemma-3-4b, nemotron-nano-30b) don't beat the lexical baseline. Quality jumps at 120B+. Counterintuitively, price doesn't track quality: the free 120B nemotron-super beat the paid 30B nemotron-nano by 71%.

**2. Free-tier is unstable.** qwen3.8-27b scored 1.000 on the 7 requests that succeeded — but 53% were rate-limited. gemma-4-26b was 100% geo-blocked. ultra-550b lost 33% to 503 errors. Only `nemotron-super:free` ran 15/15 without rate limits. Free models are great for prototyping; unreliable for production.

**3. System One protocol is underappreciated.** `jev-1.13` was the only model with a native choice API (not chat/completions). It returned probabilities directly — no JSON parsing from text. Latency was 0.46s, 5× faster than the next fastest model. And critically: **it uses the same protocol as our existing Laya client**, so integration was a one-line endpoint swap. Fusion, gating, calibration — all unchanged.

---

## What we chose

### For inline UX (latency-critical)

**Hybrid: jev-1.13 primary + glm-5.3-flash fallback + lex safety-net**

- `jev-1.13` on every request (0.46s, p@1=0.733)
- If `confidence < 0.5`, retry with `glm-5.3-flash` (p@1=1.0, 10.5s)
- On error/timeout, fall back to lex (p@1=0.404, instant)
- Cost: ~$0.08/1000 requests

### For batch/offline (quality-critical)

`glm-5.3-flash` — p@1=1.000, $0.121/1000. Good for catalog embedding generation, auto-categorization.

### For zero-cost MVP

`nemotron-super:free` + lex fallback. p@1=0.800, $0, with rate-limit risk.

---

## Pitfalls

### 1. WASM can't call APIs directly

CanvasDesk compiles to wasm for web. An API key in a public bundle leaks. Solution: backend proxy (desktop: sidecar, web: cloud function).

### 2. Latency kills inline UX

10.5s (glm-5.3-flash) is unacceptable without prefetch. Solution: request on node focus, lex-suggestion instantly, LLM-refinement later. `jev-1.13` (0.46s) solves this — you can call it on every keystroke.

### 3. Non-determinism breaks golden tests

LLMs with `temperature=0` aren't 100% deterministic. Solution: disable LLM-mm in CI, test lex only.

### 4. Hallucinations

LLMs can return a template_id not in the catalog. Validate `choice ∈ catalog`, else fallback to lex.

---

## What it costs

| Scenario | Requests/month | Cost/month |
|---|---|---|
| Light (5 edits/day) | 150 | $0.01 |
| Medium (20 edits/day) | 600 | $0.05 |
| Power user (100 edits/day) | 3,000 | $0.24 |
| Team of 10 power users | 30,000 | $2.43 |

Even a power user costs pennies. Free-tier is $0, but with rate limits.

---

## Takeaways

1. **LLM works as a ranking source.** +57–114% over baseline without fine-tuning.
2. **Scale matters.** Under 120B params, don't bother for choice-ranking tasks.
3. **Free-tier is unstable.** Only one of five free models ran 15/15 without rate limits.
4. **System One protocol is underappreciated.** `jev-1.13` at 0.46s with native choice format is ideal for inline UX — and integrates without rewriting fusion.
5. **Hybrid beats single-source.** One model doesn't cover all scenarios. jev-1.13 for speed, glm-5.3-flash for quality, lex for reliability.

Full technical report — in [dev-researches/llm-mm-source-benchmark.md](https://github.com/danku13/CanvasDesk/blob/main/docs/dev-researches/llm-mm-source-benchmark.md). Questions welcome.
