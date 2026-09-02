# Butler

Butler is a Tauri desktop shell for discovering, organizing, opening, and terminating remote Code Server sessions launched through Open OnDemand.

This repository currently contains the first runnable application slice:

- a Tauri 2 desktop scaffold,
- a React and TypeScript sidebar shell,
- project-grouped session cards,
- hardware and remaining-runtime pills,
- session selection and empty/error states,
- a confirmed kill-session action,
- and a narrow Rust command boundary backed by demo data.

The SSH, Open OnDemand, Slurm, tunnel, and child-WebView integrations are **not connected yet**. The current backend intentionally uses `MockClusterService` so the shell can be exercised before university-specific connection details are added.

## Stack

- Tauri 2
- Rust
- React 19
- TypeScript
- Vite

## Prerequisites

Install the prerequisites for your operating system from the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/). At minimum, development requires:

- Node.js 20.19+ or 22.12+
- npm
- the stable Rust toolchain installed through `rustup`
- the platform WebView and native build dependencies listed by Tauri

## Install

```bash
npm install
```

## Run the desktop app

```bash
npm run tauri dev
```

Tauri starts Vite automatically and opens the native Butler window.

## Run the UI in a browser

```bash
npm run dev
```

Open `http://localhost:1420`. Browser mode uses a local copy of the demo sessions because Tauri IPC is unavailable outside the desktop runtime. This is useful for quick layout work; it does not exercise Rust commands.

## Check and build

Type-check the frontend:

```bash
npm run check
```

Build the frontend and native application bundles:

```bash
npm run tauri build
```

Generated frontend assets are written to `dist/`; Rust artifacts and installers are written below `src-tauri/target/`.

## Current architecture

```text
React UI
   │
   │ list_sessions / kill_session
   ▼
Tauri command layer
   │
   ▼
ClusterService
   │
   └── MockClusterService (current)

Future replacement:
ClusterService
   └── OpenSSH + Slurm + Open OnDemand implementation
```

The frontend can request explicit session operations only. It cannot submit arbitrary shell commands. `ClusterService` is the seam where the demo implementation will be replaced with the university-specific SSH and scheduler adapter.

## Next implementation slice

The next milestone is the technical spike described in `DESIGN.md`:

1. load university SSH configuration,
2. establish or reuse an OpenSSH control connection,
3. identify one active OOD Code Server allocation from Slurm,
4. read that session's `connection.yml` on demand,
5. open a dynamic localhost tunnel,
6. create a Tauri child WebView in the editor region,
7. verify the Code Server authentication flow.

Credentials must remain in memory, must never be written to local metadata, and must never be logged.

## Repository guide

- `src/` — React application shell and Tauri IPC client
- `src-tauri/src/commands.rs` — narrow command surface exposed to the frontend
- `src-tauri/src/cluster.rs` — `ClusterService` boundary and current demo implementation
- `src-tauri/src/models.rs` — serializable session domain models
- `src-tauri/tauri.conf.json` — Tauri 2 window, build, and bundle configuration
- `DESIGN.md` — product and implementation plan
- `AGENTS.md` — contributor guidelines
