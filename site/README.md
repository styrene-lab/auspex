# Auspex site scaffold

This is a preliminary Astro site scaffold intended for future Cloudflare Pages hosting.

Current policy:
- buildable locally and in CI
- not launched yet
- no production Pages project should be assumed from this directory alone

## Commands

```bash
npm install
npm run dev
npm run build
```

## Planned host

Target host: `auspex.styrene.io`

That domain is declarative here for configuration symmetry only. This change does not create or launch a Cloudflare Pages project.

## GitHub Actions prep workflow

This repo includes a manual-only workflow at `.github/workflows/site-pages-prep.yml`.

It:
- installs site dependencies
- builds the static Astro output
- uploads the `site/dist/` artifact

It does **not**:
- deploy to Cloudflare Pages
- create a Pages project
- publish production content
