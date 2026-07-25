# Scout MIB Browser

A fast, flexible, and free SNMP MIB browser built with Svelte + Rust via Tauri.

## Prerequisites

- **Node.js** 18+
- **Rust** (latest stable) — [rustup.rs](https://rustup.rs)
- **System dependencies** for Tauri on Linux:

```bash
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file librsvg2-dev
```

## Development

```bash
npm install          # install JS/Rust deps (Rust compiles on first run)

npm run dev          # full app — Svelte + Rust with hot reload
```

The Tauri CLI handles orchestration: it starts the Vite dev server, then launches the Rust backend. Changes to either layer trigger a reload.

## Build Commands

| Script | Command | Description |
|--------|---------|-------------|
| `dev` | `npm run dev` | Full app in development mode (hot reload) |
| `build` | `npm run build` | Release build — generates installers/bundles |
| `tauri:debug` | `npm run tauri:debug` | Unoptimized full build for fast iteration |
| `dev:web` | `npm run dev:web` | Frontend-only (Vite dev server) |
| `build:web` | `npm run build:web` | Frontend-only production bundle |

## Checks

```bash
npm run check        # TypeScript + Svelte type checking
npm run check:rust   # Rust compilation check (no linking, fast)
```
