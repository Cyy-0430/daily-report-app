# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

A **Tauri 2** desktop app that generates formatted daily/weekly work reports. It auto-collects the day's activity from local AI-coding-tool chat logs (Claude Code, ZCode, Codex, OpenCode), then streams a Markdown report from any OpenAI-compatible API; manual bullet points and a customizable prompt template compose with the collected text. Supports edit / copy / export, SQLite history, custom themes (preset 「纸墨」/ Editorial Paper), auto-update checks (proxy-aware), and a frameless window.

Stack: **SvelteKit (SPA, adapter-static) + Svelte 5 runes + TypeScript + Tailwind v4** on the frontend; **Rust** (`reqwest` rustls + Tauri Channel streaming, `rusqlite`/SQLite for persistence) on the backend. Package manager is **pnpm** (a hook blocks `npm` — see below). Node ≥ 24.

## Commands

```bash
pnpm install              # install deps
pnpm tauri dev            # full dev: Rust backend + Vite frontend, opens window (devUrl :1420)
pnpm tauri build          # production bundle (installers)

pnpm check                # frontend type-check: svelte-kit sync && svelte-check
pnpm test                 # frontend unit tests (vitest, jsdom; tests live in src/lib/*.test.ts)
pnpm test -- src/lib/theme.test.ts   # single vitest file
pnpm format               # prettier --write (also runs via pre-commit lint-staged)
pnpm format:check         # prettier --check (CI-enforced, see .github/workflows/format-check.yml)

# Rust (run inside src-tauri/)
cargo test                # all Rust unit tests (collectors, db, config round-trips, llm)
cargo test <name>         # single test, e.g. `cargo test allowed_exclude_overrides_include`
cargo check               # fast Rust compile check
cargo fmt                 # rustfmt (CI-enforced on *.rs)
```

There is no ESLint; "lint" == `pnpm check` + `pnpm format:check` + `cargo test` + `cargo fmt`. A husky pre-commit hook runs lint-staged (prettier on staged front-end files, rustfmt on staged `.rs`).

**Release:** update `version` in `package.json` (CI syncs it into `src-tauri/tauri.conf.json` from the `v*` tag); pushing the tag triggers `.github/workflows/release.yml`, which builds Windows/macOS/Linux installers to a draft GitHub Release. The in-app updater (`plugin-updater`) checks those releases.

**Use pnpm, not npm:** a `PreToolUse: Bash` hook (`.claude/hooks/prevent-npm.py`) intercepts `npm …` commands. The lockfile is `pnpm-lock.yaml`; `tauri.conf.json`'s `beforeDevCommand`/`beforeBuildCommand` call `pnpm`.

## Architecture

The frontend and backend talk only through Tauri `invoke` commands registered in `src-tauri/src/lib.rs` (the single IPC surface). Every command has a thin typed wrapper in `src/lib/bindings.ts`.

**IPC commands** (`#[tauri::command]`, all registered in `lib.rs::run`):
- `config.rs`: `load_config`, `save_config`
- `db.rs`: `list_history`, `add_history`, `remove_history`
- `llm.rs`: `test_connection`, `generate_report`, `generate_weekly_report`
- `collector/mod.rs`: `collect_conversations`, `collect_conversations_range`, `default_collect_paths`
- `export.rs`: `export_report`, `write_text_file`

**Persistence** — `rusqlite` (SQLite, `bundled`) via a single `Mutex<Connection>` held as Tauri `State` (`db.rs`). Three tables: `history` (id/date/title/input/output/created_at; `created_at` is **seconds**), `config` (KV; each value is a JSON-serialized field), `meta` (`schema_version`, `migrated_from_store`). `AppConfig` (`config.rs` / `bindings.ts`) **does not hold history** — history is accessed only via the three db commands. On first launch the old `tauri-plugin-store` `data.json` is migrated into SQLite once, idempotently (meta-flagged). All struct fields are `#[serde(default)]` so old stores upgrade in place — **when adding a config field, default it** or existing users' `load_config` will not round-trip. See `.trellis/spec/backend/storage-spec.md`.

**Flow 1 — generate (streaming):** `+page.svelte` creates a `Channel<StreamChunk>` and calls `generateReport(input, conversations, onMessage)` → Rust `generate_report` loads config, builds the prompt, POSTs to the OpenAI-compatible endpoint (built by `llm::build_endpoint`, which auto-appends `/v1/chat/completions`), parses SSE `data:` lines for `choices[0].delta.content`, and pushes `StreamChunk` back through the channel (no retry). On success it **saves a `HistoryItem`** via `db::add_history` and returns the item so the frontend can update its history store without a refetch. `StreamChunk` is `#[serde(tag="type")]` with variants `delta`/`done`/`error`/`progress` — mirrored exactly as a TS discriminated union in `bindings.ts`.

**Flow 2 — collect (no LLM, no tokens):** `collect_conversations(date, tools, filter, toolPaths)` runs all selected collectors (`Collector` trait, registered in `all_collectors()`) — Claude Code & Codex scan JSONL dirs, ZCode & OpenCode read single SQLite db files — filters to the target local date, and renders the result into the `{{conversations}}` template variable. `toolPaths` overrides each tool's data-source path (empty/missing = default; `~` expanded server-side by `collector::expand_home`). `collect_conversations_range` does the same per-day over a date range (kept even for empty days) for the weekly flow; `default_collect_paths` feeds the settings page. See **Collector contracts** below.

**Flow 3 — weekly report (map-reduce):** `generate_weekly_report(start, end, tools, filter, toolPaths, weeklyInput, onMessage)` range-collects, then runs per-day summaries (map, templates `weeklyMapTemplate`) and one whole-week aggregation (reduce, template `weeklyReduceTemplate`, variables `{{day_summaries}}` + `{{date_range}}`). `progress` chunks (`stage: 'map' | 'reduce'`, current/total) drive the frontend progress UI; final output streams as `delta`s and is saved to history like a daily report. Both map and reduce retry up to 3× with exponential backoff — but **only before any delta has been emitted** (retrying mid-stream would duplicate output on the frontend); a failed map day is skipped, a failed reduce aborts. `/weekly` can also preview the range collect + token estimate without any LLM call.

**Prompt templates** (`template.ts` defaults + user-editable copies in config): the daily template supports `{{input}}`, `{{conversations}}`, `{{date}}` (rendered `M.D`); each weekly template has its own variables (server-side, `llm::render_*`).

**Frontend routing:** SvelteKit SPA (`ssr = false`, adapter-static with `index.html` fallback). Four routes: `/` (daily), `/weekly`, `/settings`, `/history`. `+layout.svelte` owns the frameless titlebar (`data-tauri-drag-region`, custom min/max/close buttons), nav, toast, the global `UpdateDialog`, and on mount runs `initConfig()` then `applyTheme(resolveColors(...))` and the optional auto update check. Global cross-page state lives in `src/lib/store.ts` (`config`, `history`, `toast`, `pendingInput` — writable stores). Page work state uses **module-level `$state` singletons** in `*.svelte.ts` (`report-state.svelte.ts` for daily/weekly, `theme-state.svelte.ts` for theme editing): SvelteKit client navigation doesn't re-execute modules, so state survives route switches and in-flight async writes keep accumulating; such modules can only export `const` `$state` objects (mutate properties, never reassign). Components use **Svelte 5 runes** (`$state`/`$derived`/`$props`) for local state.

**Window lifecycle** (`lib.rs`): the window is created `visible: false` and shown only on first real page load (`MAIN_WINDOW_SHOWN` AtomicBool; 3s timeout fallback; hides WebView2 cold-start blank). `tauri-plugin-single-instance` focuses the existing window on relaunch; `tauri-plugin-window-state` restores position/size/maximized only — **not** visibility (which would break the delayed-show scheme).

## Collector contracts (important — read before touching `src-tauri/src/collector/`)

Executable spec: `.trellis/spec/backend/collector-spec.md`. The invariants below are the load-bearing ones:

- **Decode, don't cast raw events.** Session files are append-only logs (JSONL or SQLite). Each collector decodes them internally into typed `ConversationLine` projections; filtering/rendering consume only that type.
- **Time filter by line `timestamp`, never by file mtime** — sessions accumulate across days. `timestamp` is UTC, converted to local before comparing to the target date.
- **Path filtering uses the session's real `cwd` field, NEVER the encoded directory name.** Directory names encode `:`/`\`/`/` as `-` and are ambiguous (`D:\work` and `D:\workplace` both encode to `D--work…`). `norm()` lowercases and unifies separators to `\`; matching is `Path::starts_with` component-prefix (so `work` does not match `workplace`, and `D:\work` matches `D:\work\sub`). **Exclude (blacklist) wins over include (whitelist)**; empty rules = no filtering.
- **Add a new tool:** implement the `Collector` trait (`id`/`display_name`/`default_path`/`collect`) and register it in `all_collectors()` — then mirror its id in the frontend `COLLECT_TOOLS` table in `bindings.ts` (and `DEFAULT_TOOL_IDS` derives from it). Tool-id sets must stay aligned on both sides.

## Cross-layer conventions

- **Rust struct ↔ TS interface must stay in sync.** Command param/result structs use `#[serde(rename_all = "camelCase")]` and are hand-mirrored as TS interfaces in `src/lib/bindings.ts` (e.g. `PathFilterParam` ↔ `PathFilter`, `StreamChunk`, the `AppConfig` tree). There is no codegen — adding/changing a field means editing both sides and the `invoke` call's keys.
- **Security boundary:** the API key never reaches the JS runtime. All LLM calls go through Rust (`reqwest` with `rustls`); the key is only present in the Rust `ApiConfig` and the settings form. The HTTP(S) `proxy` field deliberately lives on `ApiConfig` (not top-level) so `test_connection` can test unsaved form values and the updater can reuse it.
- **Markdown rendering** (`src/lib/markdown.ts`): `marked` (gfm + breaks) then `DOMPurify.sanitize` before injecting via `{@html}`.
- Before modifying any constant/config field, **search first** — see `.trellis/spec/guides/index.md` (pre-modification rule). Shared constants live in single sources: collector ids/timestamp formats in `collector/mod.rs`, app metadata in `src/lib/app-meta.ts`.

## Trellis workflow

This repo runs the **Trellis** task/spec workflow. SessionStart/UserPromptSubmit hooks inject workflow state and `.trellis/spec/` context automatically — you do not need to load it manually. Filled specs live under `.trellis/spec/backend/` (collector, storage, network, quality, …), `.trellis/spec/frontend/` (state-management, theming, component/type-safety, …), and `.trellis/spec/guides/`. Task artifacts live under `.trellis/tasks/`; Python helpers at `.trellis/scripts/` (e.g. `task.py`).

## Design system

"Editorial Paper" (user-facing name 「纸墨」, preset id `editorial-paper`): warm-paper background (`--paper`), ink body text (`--ink` / `--ink-soft` / `--ink-faint`), single terracotta accent (`--accent`). Monospace (`--mono`) for labels, counts, dates, numbers. Two main panels share a mirrored head/body/foot structure for strict alignment. The 10 CSS variables (`THEME_VAR_GROUPS` in `src/lib/theme.ts`, keys without `--`, values `#rrggbb`) are user-customizable as themes stored in `config.themeConfig` (import/export as JSON); missing keys fall back to the preset per-variable. Defined in `src/app.css`; respect these CSS variables rather than hardcoding colors.
