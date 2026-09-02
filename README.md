# Butler

Butler is a Tauri desktop shell for discovering, opening, and terminating remote Code Server sessions launched through Open OnDemand.

The current application includes a real OpenSSH, Slurm, and Open OnDemand backend. It can:

- reuse the machine's existing OpenSSH configuration, keys, agent, and jump-host rules;
- discover active OOD sessions by correlating OOD's session database with `squeue`;
- display scheduler-reported CPU, memory, GPU, partition, and remaining runtime;
- fetch `connection.yml` only when a session is opened;
- establish and reuse dynamic localhost SSH tunnels;
- close tunnels when sessions disappear or the app exits; and
- cancel the remote allocation with `scancel` from the trash action.

Friendly-name persistence, project metadata, the child Code Server WebView, and automatic Code Server login remain separate follow-on slices. The Rust `open_session` command already returns the local tunnel URL and the ephemeral Code Server password needed for that integration.

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

Butler reads JSON configuration from the Tauri application-config directory as `config.json`. When configuration is missing, the app's error banner reports the exact expected path.

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

Background SSH commands cannot answer prompts that require a terminal. Key- or agent-based authentication works directly. When the cluster requires terminal-only MFA, configure an explicit absolute `controlPath`, authenticate a master connection once, and let Butler reuse it:

```bash
ssh -M -N -f \
  -o ControlMaster=yes \
  -o ControlPersist=600 \
  -o ControlPath=/absolute/path/to/butler-%C.sock \
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

Opening a running session performs a separate on-demand operation:

1. locate the session's staged directory in either the standard or per-cluster OOD layout;
2. read its `connection.yml`;
3. parse the host, port, optional path, and generated password;
4. reserve an unused localhost port;
5. start `ssh -N -L` with `ExitOnForwardFailure=yes`; and
6. return a localhost URL for the future child WebView.

## Backend commands

The frontend has a narrow Tauri command surface:

- `backend_status()`
- `list_sessions()`
- `open_session(session_id)`
- `close_session(session_id)`
- `kill_session(session_id)`

The frontend cannot submit arbitrary local or remote commands. Scheduler cancellation uses the job ID from Butler's trusted OOD/Slurm correlation cache rather than accepting a raw job ID from JavaScript.

## Credential handling

Butler does not write Code Server passwords to disk or include them in refresh results. A password is read only by `open_session`, returned through Tauri IPC for the upcoming authentication step, and not retained in the tunnel registry. It is never included in command strings or logs.

Local configuration should contain cluster routing only, not passwords or OOD session secrets.

## Run the desktop app

```bash
npm run tauri dev
```

## Run the UI in a browser

```bash
npm run dev
```

Open `http://localhost:1420`. Browser mode still uses local demo sessions because Tauri IPC and SSH are unavailable outside the desktop runtime.

## Check and build

```bash
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

Generated frontend assets are written to `dist/`; Rust artifacts and installers are written below `src-tauri/target/`.

## Repository guide

- `src/` — React application shell and Tauri IPC client
- `src-tauri/src/config.rs` — config-file and environment loading
- `src-tauri/src/ssh.rs` — controlled OpenSSH execution and forwarding
- `src-tauri/src/cluster.rs` — OOD/Slurm discovery, parsing, tunnels, and cancellation
- `src-tauri/src/commands.rs` — narrow asynchronous Tauri command surface
- `src-tauri/src/models.rs` — serializable backend models
- `config.example.json` — cluster configuration template
- `DESIGN.md` — product and implementation plan
- `AGENTS.md` — contributor guidelines

## Cluster-specific information still needed

To test Butler against a particular university deployment, supply the SSH target or alias and the exact OOD app token. If the defaults do not discover the session, the useful diagnostics are a **redacted** OOD database record and a **redacted** `connection.yml`; remove the password and any other secret before sharing them.
