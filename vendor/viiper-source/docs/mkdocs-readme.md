# Zensical Documentation

This directory contains the Zensical-based documentation for VIIPER (drop-in MkDocs replacement, see `mkdocs.yml`).

## Setup

Install Zensical (bundles the Material theme):

```bash
pip install -r requirements.txt
# or
pip install zensical
```

## Development

Run the documentation server locally:

```bash
zensical serve
```

Then open http://127.0.0.1:8000/ in your browser.

## Building

Build the static documentation site:

```bash
zensical build --strict
```

The built site will be in the `site/` directory.

## Deployment

Deployment is automated via GitHub Actions (`.github/workflows/docs-deploy.yml`):

```bash
zensical build --strict
```

The `site/` directory is uploaded as a Pages artifact and deployed to GitHub Pages.

## Documentation Structure

- `mkdocs.yml` - MkDocs configuration
- `docs/` - Documentation source files (Markdown)
    - `index.md` - Home page
    - `getting-started/` - Installation and quick start
    - `libviiper/` - libVIIPER API overview and integration guides
    - `devices/` - Device-specific documentation
