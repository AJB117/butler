# Butler

Butler is a Tauri desktop shell for discovering, opening, and terminating remote Code Server sessions launched through Open OnDemand.

The current application can:

- reuse the machine's existing OpenSSH configuration, keys, agent, and jump-host rules;
- discover active Open OnDemand sessions by correlating OOD's session database with `squeue`;
- display scheduler-reported CPU, memory, GPU, partition, and remaining runtime;
- retrieve `connection.yml` only when a session is opened;
- establish and reuse dynamic localhost SSH tunnels;
- embed the selected Code Server editor in a native Tauri child WebView;
- authenticate Code Server without placing its generated password in the URL;
- keep up to five recently used editors warm for fast session switching;
- isolate each editor in an ephemeral browser profile so localhost authentication cookies do not collide;
- resize, hide, and clean up child WebViews as the window and session list change; and
- cancel the remote allocation with `scancel` from the trash action.

Friendly-name persistence and project metadata remain separate follow-on slices.

## Stack

- Tauri 2
- Rust
- React 19
- TypeScript
- Vite
- the operating system's OpenSSH client
- Slurm and Open OnDemand on the remote cluster

## Prerequisites

Install the platform prerequisites from the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/). Development also requires:

- Node.js 20.19+ or 22.12+
- npm
- a stable Rust toolchain
- `ssh` installed locally
- an SSH target or alias that reaches the cluster login node
- access to `squeue` and `scancel` after login
- the university VPN when the cluster requires it

Verify the underlying connection before starting Butler:

```bash
ssh your-cluster-ssh-alias true
```

## Install

```bash
npm install
```

## Configure the cluster backend

Butler reads JSON configuration from the Tauri application-config directory as `config.json`. When configuration is missing, the app reports the exact expected path.

For development, it is usually easier to keep an ignored local file and point Butler at it:

```bash
cp config.example.json config.local.json
```

Set at least `ssh.target` in `config.local.json`, then run:

```bash
BUTLER_CONFIG="$PWD/config.local.json" npm run tauri dev
```

On PowerShell:

```powershell
Copy-Item config.example.json config.local.json
$env:BUTLER_CONFIG = "$PWD\config.local.json"
npm run tauri dev
```

### Configuration reference

```json
{
  "ssh": {
    "target": "your-cluster-ssh-alias",
    "binary": "ssh",
    "configFile": null,
    "port": null,
    "connectTimeoutSeconds": 10,
    "commandTimeoutSeconds": 30,
    "controlPersistSeconds": 600,
    "multiplex": true,
    "controlPath": null
  },
  "ood": {
    "dataRoot": "~/ondemand/data/sys/dashboard",
    "appTokens": []
  },
  "slurm": {
    "squeueBinary": "squeue",
    "scancelBinary": "scancel"
  }
}
```

`ssh.target` may be a hostname, `user@host`, or an alias from `~/.ssh/config`. `configFile` and `port` are optional. Unix builds enable OpenSSH multiplexing by default; Windows builds do not.

Butler runs remote commands through `/bin/bash -lc`, so the cluster's login-shell PATH and normal shell initialization are available to `squeue` and `scancel`. If a deployment still does not expose Slurm in that environment, set `slurm.squeueBinary` and `slurm.scancelBinary` to their absolute remote paths.

`ood.dataRoot` defaults to Open OnDemand's standard dashboard data root. `ood.appTokens` is an optional exact allowlist of OOD app tokens such as `sys/bc_code_server`. When it is empty, Butler includes records whose token or title contains a common Code Server or VS Code spelling.

The following environment variables override the file:

| Variable | Purpose |
| --- | --- |
| `BUTLER_CONFIG` | Alternate JSON config path |
| `BUTLER_SSH_TARGET` | SSH hostname, `user@host`, or alias |
| `BUTLER_SSH_BINARY` | Local OpenSSH executable |
| `BUTLER_SSH_CONFIG` | Alternate local SSH config file |
| `BUTLER_SSH_PORT` | SSH port |
| `BUTLER_SSH_MULTIPLEX` | Enable or disable control connections |
| `BUTLER_SSH_CONTROL_PATH` | Explicit OpenSSH control-socket path |
| `BUTLER_OOD_DATA_ROOT` | Remote OOD dashboard data root |
| `BUTLER_OOD_APP_TOKENS` | Comma-separated exact OOD app tokens |
| `BUTLER_SQUEUE_BINARY` | Remote `squeue` executable or path |
| `BUTLER_SCANCEL_BINARY` | Remote `scancel` executable or path |

### Interactive MFA

Background SSH commands cannot answer prompts that require a terminal. Key- or agent-based authentication works directly. When the cluster requires terminal-only MFA, configure a short absolute `controlPath`, authenticate a master connection once, and let Butler reuse it:

```bash
ssh -M -N -f \
  -o ControlMaster=yes \
  -o ControlPersist=600 \
  -o ControlPath=/tmp/butler-%C \
  your-cluster-ssh-alias
```

Use the same `target`, `controlPersistSeconds`, and `controlPath` in `config.local.json`. Re-authentication is required after that master connection expires or the network changes.

## How discovery works

Butler does not probe every historical `connection.yml` file. Each refresh performs one SSH operation that:

1. reads the small JSON records below `<dataRoot>/batch_connect/db`;
2. asks Slurm for this user's pending, configuring, running, and completing jobs;
3. correlates OOD records with scheduler jobs by job ID; and
4. returns only active Code Server sessions.

A routine refresh never reads or returns Code Server passwords.

## How opening an editor works

Selecting a running session performs the following sequence:

1. locate the session's staged directory in either the standard or per-cluster OOD layout;
2. read its `connection.yml` over SSH;
3. parse the compute host, port, optional path, and generated password;
4. reserve an unused localhost port;
5. start `ssh -N -L` with `ExitOnForwardFailure=yes`;
6. create a child WebView over the editor region;
7. send the localhost URL and password to a bundled local bootstrap page through an in-memory Tauri event;
8. submit the normal Code Server login form; and
9. navigate the child WebView to the live editor.

The bootstrap accepts only loopback HTTP or HTTPS URLs. The password is sent as form data rather than placed in the URL, persisted in Butler's configuration, or logged. Remote Code Server pages are not granted Butler's Tauri IPC permissions.

Each child WebView uses an isolated, ephemeral browser profile. This is necessary because browser cookies are scoped to the localhost host rather than its port; separate profiles prevent one tunneled Code Server session from overwriting another session's authentication cookie.

Butler caches the five most recently used editors. Switching sessions hides the current child WebView and reveals the selected cached WebView without reloading Code Server. Evicting an editor closes both its WebView and SSH tunnel. Expired or killed sessions are pruned automatically.

## Backend commands

The frontend has a narrow Tauri command surface:

- `backend_status()`
- `list_sessions()`
- `open_session(session_id)`
- `close_session(session_id)`
- `kill_session(session_id)`

The frontend cannot submit arbitrary local or remote commands. Scheduler cancellation uses the job ID from Butler's trusted OOD/Slurm correlation cache rather than accepting a raw job ID from JavaScript.

## Credential handling

Butler does not write Code Server passwords to disk or include them in refresh results. A password is read only by `open_session`, delivered to the local editor bootstrap through a targeted in-memory event, placed briefly in a hidden POST form, and cleared from Butler's JavaScript objects after dispatch.

The resulting Code Server cookie remains inside that child WebView's ephemeral isolated profile. Local configuration should contain cluster routing only, not passwords or OOD session secrets.

## Run the desktop app

```bash
npm run tauri dev
```

Selecting a running session should open Code Server directly inside the workspace. The first selection creates a tunnel and child WebView; subsequent selections of a cached session should be nearly immediate.

## Run the UI in a browser

```bash
npm run dev
```

Open `http://localhost:1420`. Browser mode uses demo sessions and displays a desktop-required message because Tauri child WebViews, IPC, and SSH are unavailable in a normal browser tab.

## Check and build

```bash
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

Vite builds both `index.html` for the main UI and `editor.html` for the local child-WebView bootstrap. Generated frontend assets are written to `dist/`; Rust artifacts and installers are written below `src-tauri/target/`.

## Repository guide

- `src/` — React application shell and Tauri IPC client
- `src/components/EditorHost.tsx` — DOM host and editor opening/error state
- `src/lib/editorWebviews.ts` — child-WebView cache, sizing, switching, and cleanup
- `src/editor.ts` and `editor.html` — local Code Server authentication bootstrap
- `src-tauri/src/config.rs` — config-file and environment loading
- `src-tauri/src/ssh.rs` — controlled OpenSSH execution and forwarding
- `src-tauri/src/cluster.rs` — OOD/Slurm discovery, parsing, tunnels, and cancellation
- `src-tauri/src/commands.rs` — narrow asynchronous Tauri command surface
- `src-tauri/src/models.rs` — serializable backend models
- `src-tauri/capabilities/default.json` — permissions for the main UI and local child bootstrap
- `config.example.json` — cluster configuration template
- `DESIGN.md` — product and implementation plan
- `AGENTS.md` — contributor guidelines

## Troubleshooting

If a child editor does not open, first verify that session refresh still works and inspect the error shown in the workspace. Useful checks are:

```bash
ssh your-cluster-ssh-alias '/bin/bash -lc "command -v squeue; command -v scancel"'
ssh your-cluster-ssh-alias true
```

For cluster-specific discovery problems, provide the exact OOD app token plus a redacted OOD database record and redacted `connection.yml`. Remove the password and any other secret before sharing them.
