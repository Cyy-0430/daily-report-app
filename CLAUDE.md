# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

A **Tauri 2** desktop app that generates formatted daily-work reports. Left panel takes today's bullet points (or auto-collects them from local Claude Code chat logs); right panel streams a Markdown report from any OpenAI-compatible API, then supports edit / copy / export. UI is a custom "Editorial Paper" theme on a frameless window.

Stack: **SvelteKit (SPA, adapter-static) + Svelte 5 runes + TypeScript + Tailwind v4** on the frontend; **Rust** (`reqwest` rustls + Tauri Channel streaming, `rusqlite`/SQLite for persistence) on the backend. Package manager is **pnpm** (there is a hook that blocks `npm` — see below).

## Commands

```bash
pnpm install              # install deps
pnpm tauri dev            # full dev: Rust backend + Vite frontend, opens window (devUrl :1420)
pnpm tauri build          # production bundle (installers)
pnpm dev / pnpm build     # frontend only (Vite); rarely run standalone — tauri wraps these

pnpm check                # type-check: svelte-kit sync && svelte-check --tsconfig ./tsconfig.json
pnpm check:watch          # watch mode

# Rust (run inside src-tauri/)
cargo test                # all Rust unit tests (collector path-filter + extract, db migration/CRUD)
cargo test <name>         # single test, e.g. `cargo test allowed_exclude_overrides_include`
cargo check               # fast Rust compile check
```

There is no ESLint configured; "lint" == `pnpm check` (svelte-check type-check) plus `cargo test`.

**Release:** pushing a `v*` tag triggers `.github/workflows/release.yml`, which syncs the version into `tauri.conf.json` and builds Windows/macOS/Linux installers to a draft GitHub Release. Update `version` in both `package.json` and `src-tauri/tauri.conf.json` (the CI overwrites the latter from the tag).

**Use pnpm, not npm:** a `PreToolUse: Bash` hook (`.claude/hooks/prevent-npm.py`) intercepts `npm …` commands. The lockfile is `pnpm-lock.yaml`; `tauri.conf.json`'s `beforeDevCommand`/`beforeBuildCommand` call `pnpm`.

## Architecture

The frontend and backend talk only through Tauri `invoke` commands registered in `src-tauri/src/lib.rs` (the single IPC surface). Every command has a thin typed wrapper in `src/lib/bindings.ts`.

**IPC commands** (`#[tauri::command]`, all registered in `lib.rs::run`):
- `config.rs`: `load_config`, `save_config`
- `db.rs`: `list_history`, `add_history`, `remove_history`
- `llm.rs`: `test_connection`, `generate_report`
- `collector/mod.rs`: `collect_conversations`
- `export.rs`: `export_report`, `write_text_file`

**Persistence** — `rusqlite` (SQLite, `bundled`) via a single `Mutex<Connection>` held as Tauri `State` (`db.rs`). Three tables: `history` (id/date/title/input/output/created_at; `created_at` is **seconds**), `config` (KV; each value is a JSON-serialized field), `meta` (`schema_version`, `migrated_from_store`). `AppConfig` (`config.rs` / `bindings.ts`) **no longer holds history** — history is accessed only via `list_history`/`add_history`/`remove_history`. On first launch, the old `tauri-plugin-store` `data.json` (key `config`) is migrated into SQLite once, idempotently (meta-flagged), and the original file is kept as a fallback. All struct fields are `#[serde(default)]` so old stores upgrade in place — **when adding a config field, default it** or existing users' `load_config` will not round-trip. See `.trellis/spec/backend/storage-spec.md` for the executable contract.

**Flow 1 — generate (streaming):** `+page.svelte` creates a `Channel<StreamChunk>` and calls `generateReport(input, conversations, onMessage)` → Rust `generate_report` loads config, builds the prompt, POSTs to the OpenAI-compatible endpoint (built by `llm::build_endpoint`, which auto-appends `/v1/chat/completions`), parses SSE `data:` lines for `choices[0].delta.content`, and pushes `StreamChunk::{Delta, Done, Error}` back through the channel. On success it **saves a `HistoryItem`** via `db::add_history` (independent of config) and returns the item so the frontend can update its history store without a refetch. `StreamChunk` is `#[serde(tag="type")]` with variants `delta`/`done`/`error` — mirrored exactly as a TS discriminated union in `bindings.ts`.

**Flow 2 — collect (no LLM, no tokens):** `collect_conversations(date, tools, filter)` reads local Claude Code chat logs (`~/.claude/projects/<encoded-project>/*.jsonl`), filters to the target local date, and renders the result into the `{{conversations}}` template variable. The rendered text is what gets passed into the generate flow. See **Collector contracts** below.

**Prompt template** (`template.ts` default + user-editable copy in config): supports three variables substituted server-side by `llm::render_template`: `{{input}}` (today's bullets), `{{conversations}}` (collected chat text, may be empty), and `{{date}}` (rendered as `M.D`, e.g. `7.10`).

**Frontend routing:** SvelteKit SPA (`ssr = false`, adapter-static with `index.html` fallback). Three routes: `/` (generate), `/settings`, `/history`. `+layout.svelte` owns the frameless titlebar (`data-tauri-drag-region`, custom min/max/close buttons), the nav, the toast, and calls `initConfig()` on mount. Global state lives in `src/lib/store.ts` (`config`, `toast`, `pendingInput`). Components use **Svelte 5 runes** (`$state`/`$derived`/`$props`), not stores, for local state.

## Collector contracts (important — read before touching `src-tauri/src/collector/`)

This module has a filled executable spec at `.trellis/spec/backend/collector-spec.md`; the invariants below are the load-bearing ones:

- **Decode, don't cast raw jsonl.** Each `*.jsonl` line is one append-only event. `ClaudeCodeCollector::parse_session` produces typed `ConversationLine` projections; filtering/rendering consume only that type.
- **Time filter by line `timestamp`, never by file mtime** — sessions accumulate across days. `timestamp` is UTC RFC3339, converted to local before comparing to the target date.
- **Path filtering uses the session's real `cwd` field, NEVER the encoded directory name.** Directory names encode `:`/`\`/`/` as `-` and are ambiguous (`D:\work` and `D:\workplace` both encode to `D--work…`). `norm()` lowercases and unifies separators to `\`; matching is `Path::starts_with` component-prefix (so `work` does not match `workplace`, and `D:\work` matches `D:\work\sub`). **Exclude (blacklist) wins over include (whitelist)**; empty rules = no filtering.
- **Add a new tool:** implement the `Collector` trait (with the `filter` param) and register it in `all_collectors()` — one line.

## Cross-layer conventions

- **Rust struct ↔ TS interface must stay in sync.** Command param/result structs use `#[serde(rename_all = "camelCase")]` and are hand-mirrored as TS interfaces in `src/lib/bindings.ts` (e.g. `PathFilterParam` ↔ `PathFilter`). There is no codegen — adding/changing a field means editing both sides and the `invoke` call's keys.
- **Security boundary:** the API key never reaches the JS runtime. All LLM calls go through Rust (`reqwest` with `rustls`); the key is only present in the Rust `ApiConfig` and the settings form. The frontend holds only an empty/string config object.
- **Markdown rendering** (`src/lib/markdown.ts`): `marked` (gfm + breaks) then `DOMPurify.sanitize` before injecting via `{@html}`.
- Before modifying any constant/config field, **search first** — see `.trellis/spec/guides/index.md` (pre-modification rule).

## Trellis workflow

This repo runs the **Trellis** task/spec workflow. SessionStart/UserPromptSubmit hooks inject workflow state and `.trellis/spec/` context automatically — you do not need to load it manually. Specs of record: `.trellis/spec/backend/`, `.trellis/spec/frontend/`, `.trellis/spec/guides/` (most frontend/guide pages are templates; `collector-spec.md` is filled). Task artifacts live under `.trellis/tasks/`. The Python helpers are at `.trellis/scripts/` (e.g. `task.py`).

## Design system

"Editorial Paper": warm-paper background (`--paper`), ink body text (`--ink` / `--ink-soft` / `--ink-faint`), single terracotta accent (`--accent`). Monospace (`--mono`) for labels, counts, dates, numbers. Two main panels share a mirrored head/body/foot structure for strict alignment. Defined in `src/app.css`; respect these CSS variables rather than hardcoding colors.
