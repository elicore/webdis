# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Added
- Renamed canonical project/runtime identity to `redis-web`.
- Split monolith into a workspace with 4 crates:
  - `redis-web-core`
  - `redis-web-runtime`
  - `redis-web-compat`
  - `redis-web`
- Added Starlight docs site and pruned legacy fork-derived docs.
- Added compatibility-focused docs pages and compatibility guard checks in CI.
- Switched image build/publish to GHCR (`ghcr.io/<owner>/redis-web`) with legacy compatibility tags (`ghcr.io/<owner>/webdis`).

### Changed
- Canonical CLI binary is now `redis-web`.
- Canonical config/schema files are now `redis-web.json`, `redis-web.prod.json`, and `redis-web.schema.json`.
- Root README is minimized; deep usage and maintainer docs moved to docs site.

### Compatibility
- `webdis` alias binary remains available during a transition window.
- Legacy config filenames remain accepted (`webdis.json`, `webdis.prod.json`, `webdis.schema.json`, `webdis.legacy.json`).

### Deprecation timeline (`webdis` alias + legacy naming)
- 2026-03-02: Alias and legacy naming marked deprecated; compatibility docs and migration guide published.
- 2026-06-30 (target): Begin soft-freeze of new legacy-surface additions; no new features added under legacy names.
- 2026-09-30 (target): Remove `webdis` alias binary and legacy naming defaults in the next breaking release.

> Dates are target milestones and may shift based on downstream migration feedback.
