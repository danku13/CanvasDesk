# Content OS — Landing Demo (5 heritage styles)

A single landing page for **Content OS** (AI agents for product marketing:
strategy → tactics → implementation) rendered in five historical design movements
from the [`60-styles/`](../../60-styles/) presets:

| Style | Preset | Years |
|---|---|---|
| Ар-нуво | [`art-nouveau.md`](../../60-styles/art-nouveau.md) | 1893–1910 |
| Де Стейл | [`de-stijl.md`](../../60-styles/de-stijl.md) | 1917 |
| Баухаус | [`bauhaus.md`](../../60-styles/bauhaus.md) | 1919 |
| Конструктивизм | [`constructivism.md`](../../60-styles/constructivism.md) | ~1921 |
| Ар-деко | [`art-deco.md`](../../60-styles/art-deco.md) | 1925 |

One fixed DOM (`index.html`) + a token/skin CSS layer per style + a tiny
switcher script. See [`STRUCTURE.md`](STRUCTURE.md) for the full contract.

## Run locally

```bash
cd demo/content-os-landing
python3 -m http.server 8080
# → http://localhost:8080
```

No build step. Google Fonts is the only external dependency; system-font
fallbacks keep the page fully usable offline.

## Deploy

GitHub Actions workflow (`.github/workflows/deploy-pages.yml`) uploads this
directory to GitHub Pages on every push to `main` touching `demo/**`.

## Accessibility

Contrast pairs, focus states, reduced-motion handling and table semantics are
inherited from the presets' a11y watchpoints — see `STRUCTURE.md`. Demo copy is
in Russian; all content is illustrative.
