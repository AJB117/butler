# Butler — Tauri Code Server Session Manager Design

## Overview

Butler is a desktop application for managing remote Code Server sessions launched through a university Open OnDemand (OOD) deployment.

The goal is to replace the friction of juggling many browser tabs and unlabeled OOD sessions with a single native desktop workspace that can:

- discover active Code Server sessions,
- assign friendly names and project folders,
- display hardware and remaining runtime,
- switch between sessions from a sidebar,
- terminate sessions remotely,
- and render the selected Code Server session directly in the main application window.

The first version should **not attempt to replace Open OnDemand's session-launch flow**. OOD can remain responsible for creating scheduler jobs, while Butler becomes the primary interface for discovering, organizing, opening, and terminating them.

---

## Feasibility

All requested core features are feasible.

| Requirement | Feasible? | Notes |
| --- | --- | --- |
| Session naming | Yes | Store locally in SQLite, keyed by OOD session ID or scheduler job ID. |
| Project folders | Yes | Store a remote project path alongside local metadata. |
| Three-day expiry handling | Yes | Prefer scheduler-reported remaining runtime over calculating it locally. |
| Hardware badges | Yes | Derived from Slurm allocation details where available. Exact GPU model depends on what the cluster exposes. |
| Remaining runtime | Yes | Query the scheduler's job time limit/end time. |
| Remote session kill | Yes | Map the OOD session to its scheduler job and run `scancel <job_id>`. |
| Sidebar navigation | Yes | Native/local Tauri UI. |
| Code Server in main window | Yes | Use a child WebView rather than an iframe. |
| Automatic credential retrieval | Yes | OOD writes runtime connection information to `connection.yml`. |
| Completely invisible Code Server login | Probably | Prototype first; the university may customize authentication behavior. |

The main technical risk to spike early is **automatic authentication inside the embedded Code Server WebView**. Retrieving a generated password is straightforward; guaranteeing a completely invisible login flow depends on the university's Code Server/OOD configuration.

---

## User Experience

The application should feel like a local IDE shell rather than an alternate Open OnDemand portal.

```text
┌──────────────────── Butler ────────────────────────────────┐
│ Sidebar        │                                          │
│                │                                          │
│ ▾ Research     │          Code Server WebView             │
│   ● Training   │                                          │
│     [A100 ×1]  │                                          │
│     [2d 11h]   │                                          │
│                │                                          │
│ ▾ CS 4501      │                                          │
│   ● Homework   │                                          │
│     [8 CPU]    │                                          │
│     [18h 42m]  │                                          │
│                │                                          │
│   + Project    │                                          │
└────────────────┴──────────────────────────────────────────┘
```

### Session item

Each sidebar session should contain:

- friendly session name,
- project name,
- remote folder,
- running/pending/expired status,
- hardware pills,
- remaining-runtime pill,
- a trash-can icon for termination.

Example:

```text
┌ Research Training                              🗑 ┐
│ ~/research                                        │
│ ● Running   [ A100 ×1 ] [ 8 CPU ] [ 64 GB ]      │
│                                      [ 2d 11h ]   │
└───────────────────────────────────────────────────┘
```

Runtime pills can visually escalate as expiration approaches:

```text
[2d 18h]    normal
[3h 14m]    warning
[18m]       urgent
[Expired]   inactive
```

The scheduler should remain authoritative for expiration. If the university enforces a three-day limit, Butler should show the actual scheduler-reported remaining runtime rather than assume exactly 72 hours from creation.

---

## High-Level Architecture

```text
┌──────────────────── Tauri Application ────────────────────┐
│                                                          │
│  Local frontend                 Embedded Code Server      │
│  ┌───────────────┐             ┌───────────────────────┐ │
│  │ Sidebar       │             │ Child WebView         │ │
│  │ Projects      │             │                       │ │
│  │ Sessions      │             │ 127.0.0.1:<port>      │ │
│  │ Runtime pills │             │                       │ │
│  └───────┬───────┘             └───────────▲───────────┘ │
│          │                                  │             │
│          ▼                                  │             │
│  ┌──────────────── Rust backend ────────────┴───────────┐ │
│  │ ClusterService                                      │ │
│  │                                                     │ │
│  │ - session discovery                                 │ │
│  │ - scheduler queries                                 │ │
│  │ - SSH lifecycle                                     │ │
│  │ - local port forwarding                             │ │
│  │ - kill operations                                   │ │
│  │ - connection credential retrieval                   │ │
│  └───────────────────────┬─────────────────────────────┘ │
└──────────────────────────┼───────────────────────────────┘
                           │ SSH
                           ▼
                 University login node
                           │
                ┌──────────┴──────────┐
                │                     │
             Scheduler           OOD session data
              (Slurm)       ~/ondemand/data/.../
                │                     │
                └──────────┬──────────┘
                           ▼
                    Compute nodes
                    + Code Server
```

---

## Technology Stack

### Desktop shell

- **Tauri 2**
- Rust backend
- Web frontend (React, Solid, Svelte, or vanilla TypeScript)

The frontend framework is not critical. React or Solid would both be reasonable; the important boundary is that cluster operations live in Rust rather than directly in browser JavaScript.

### Persistence

Use local SQLite for Butler-owned metadata.

Suggested tables:

```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    remote_path TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE session_metadata (
    ood_session_id TEXT PRIMARY KEY,
    friendly_name TEXT,
    project_id INTEGER,
    last_seen_at INTEGER,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);
```

OOD and Slurm remain authoritative for runtime state. SQLite contains only user-facing organizational state.

### SSH

For the first version, use the machine's existing OpenSSH client rather than embedding a full SSH implementation in Rust.

Benefits:

- reuses `~/.ssh/config`,
- reuses existing keys,
- reuses the SSH agent,
- respects university-specific SSH configuration,
- reduces custom authentication code.

The Rust backend can spawn `/usr/bin/ssh` using tightly controlled arguments.

Do **not** expose arbitrary shell execution to the frontend.

---

## Rust Backend API

Expose a narrow Tauri command surface.

Conceptually:

```rust
#[tauri::command]
async fn list_sessions() -> Result<Vec<Session>, AppError>;

#[tauri::command]
async fn open_session(session_id: String) -> Result<OpenSessionResult, AppError>;

#[tauri::command]
async fn kill_session(session_id: String) -> Result<(), AppError>;

#[tauri::command]
async fn rename_session(session_id: String, name: String) -> Result<(), AppError>;

#[tauri::command]
async fn create_project(name: String, remote_path: Option<String>) -> Result<Project, AppError>;

#[tauri::command]
async fn assign_project(session_id: String, project_id: i64) -> Result<(), AppError>;
```

All cluster-specific behavior should sit behind a `ClusterService` abstraction.

```rust
trait ClusterService {
    async fn list_sessions(&self) -> Result<Vec<RemoteSession>>;
    async fn connection_info(&self, session_id: &str) -> Result<ConnectionInfo>;
    async fn kill_job(&self, job_id: &str) -> Result<()>;
    async fn open_tunnel(&self, connection: &ConnectionInfo) -> Result<TunnelHandle>;
}
```

This lets Butler support a UVA-specific implementation first while keeping room for other Open OnDemand deployments later.

---

## Session Discovery

### Problem with naive discovery

OOD leaves historical session directories behind, so scanning every `connection.yml` and attempting every recorded port becomes slow over time.

The application should **not** treat the OOD session directory as the primary list of active sessions.

### Preferred discovery flow

Start with the scheduler, which already knows which jobs are active.

```text
Slurm
  │
  ├── active job 123456
  ├── active job 123457
  └── active job 123458
          │
          ▼
 map job → OOD session directory
          │
          ▼
 connection.yml
          │
          ├── host
          ├── port
          └── password
```

The remote discovery command should ideally return one JSON payload per refresh rather than require many SSH round trips.

Example normalized response:

```json
[
  {
    "ood_session_id": "03a8efa7-da5f-436f-919f-617b948c1358",
    "job_id": "12345678",
    "status": "running",
    "host": "udc-aw33-21c1",
    "port": 3450,
    "started_at": "2026-09-01T12:18:00-04:00",
    "ends_at": "2026-09-04T12:18:00-04:00",
    "cpus": 8,
    "memory_mb": 65536,
    "gpus": [
      {
        "model": "A100",
        "count": 1
      }
    ]
  }
]
```

The password should generally be fetched only when opening a session and should not be included in routine polling results.

---

## SSH Connection Strategy

### Persistent control connection

Avoid creating a fresh SSH handshake for every refresh.

The ideal lifecycle is:

```text
Butler starts
   │
   ▼
authenticate to university once
   │
   ▼
persistent SSH control connection
   │
   ├── query scheduler
   ├── read OOD metadata
   ├── create tunnels
   └── kill jobs
```

OpenSSH multiplexing can provide this without implementing SSH protocol handling in Rust.

A control socket can be created using a command conceptually similar to:

```bash
ssh \
  -M \
  -S /tmp/butler-%C.sock \
  -N \
  user@login.cluster.edu
```

Follow-up operations can reuse it with `-S`.

The implementation must handle:

- VPN disconnected,
- hotspot/network interruption,
- SSH timeout,
- expired MFA/authentication,
- control connection death,
- reconnect behavior.

These errors should surface as app-level connection state rather than raw terminal output.

---

## Opening a Session

Selecting a session should perform the following sequence:

```text
User selects session
        │
        ▼
Fetch connection.yml
        │
        ├── host
        ├── port
        └── password
        │
        ▼
Allocate unused localhost port
        │
        ▼
Open SSH tunnel
        │
        ▼
127.0.0.1:<local-port>
        │
        ▼
remote compute node:<code-server-port>
        │
        ▼
Create/show Code Server WebView
```

Example forwarding topology:

```text
Butler WebView
     │
     │ http://127.0.0.1:18103
     ▼
localhost:18103
     │
     │ encrypted SSH tunnel
     ▼
login.hpc.example.edu
     │
     ▼
compute-node:3450
     │
     ▼
Code Server
```

The local port is arbitrary and should be dynamically allocated.

---

## WebView Model

Do not use an iframe for the main editor.

Use a Tauri child WebView positioned beside the sidebar.

Recommended behavior:

1. The main/local webview renders Butler's navigation and session list.
2. Selecting a session creates or reveals a child WebView.
3. The child WebView navigates to that session's localhost tunnel URL.
4. Changing sessions hides the current child WebView and shows another.

### Session WebView cache

Keep recently used Code Server WebViews alive so switching sessions does not force Code Server to reload every time.

Example policy:

- create lazily on first open,
- retain the five most recently used session WebViews,
- destroy least-recently-used WebViews beyond the limit,
- keep SSH tunnels alive while their WebViews are cached,
- close the tunnel when its WebView is evicted or its remote session expires.

This gives fast `Research → Homework → Research` switching while keeping memory use bounded.

---

## Code Server Authentication

OOD's session output commonly contains a generated Code Server password in `connection.yml`.

Butler should:

1. fetch the password only when opening a session,
2. keep it in memory only as long as necessary,
3. never write it to SQLite,
4. never log it,
5. discard it after authentication succeeds.

### Authentication spike

Before building the full application, verify that the embedded WebView can authenticate automatically against the university's Code Server configuration.

Prototype flow:

```text
Discover one live session
        ↓
Read host / port / password
        ↓
Open SSH tunnel
        ↓
Create child WebView
        ↓
Authenticate automatically
        ↓
Editor is usable
```

Potential strategies, in order of preference:

1. Reproduce the normal Code Server login POST programmatically in the WebView session.
2. Inject the credential into the login form and submit it.
3. If university-specific behavior prevents safe automation, show the Code Server login page while making the password available through a one-click local UI action.

The application should not depend on copying credentials manually from OOD.

---

## Session Naming and Projects

OOD sessions have UUIDs that are useful as stable identifiers but poor UX.

Butler should keep a local mapping:

```text
03a8efa7-da5f-... → "Research Training"
f56f937c-a385-... → "CS 4501 Homework"
```

Projects provide a second organizational layer.

```text
▾ Research
    Training
    Dataset cleanup
    Evaluation

▾ CS 4501
    Homework 3
    Final project

▾ Misc
    Scratch
```

Each project may have a default remote path:

```text
Research
→ /home/user/projects/research
```

A session may override that path if needed.

Recommended session context menu:

```text
Rename
Move to Project
Change Folder
Open in Browser
Copy Session ID
Kill Session
```

Drag-and-drop project organization can be added later; it is not required for the MVP.

---

## Hardware Metadata

Scheduler information should be normalized into human-readable pills.

Raw values might resemble:

```text
cpu=8
mem=64G
gres/gpu:a100:1
```

Butler should normalize these to:

```text
[8 CPU] [64 GB] [A100 ×1]
```

Suggested model:

```rust
struct HardwareAllocation {
    cpus: Option<u32>,
    memory_bytes: Option<u64>,
    gpus: Vec<GpuAllocation>,
    partition: Option<String>,
}

struct GpuAllocation {
    model: Option<String>,
    count: u32,
}
```

If the scheduler exposes only a partition or constraint rather than the exact GPU model, Butler should display what is known instead of guessing.

Examples:

```text
[GPU ×1]
[gpu partition]
[A100 ×1]
```

A university-specific mapping table can improve friendly hardware names later.

---

## Remaining Runtime

Use scheduler data as the source of truth.

The UI should derive remaining runtime from either:

- scheduler end time, or
- scheduler elapsed time + configured time limit.

Do not assume a job lasts exactly three days simply because three days is the maximum.

Suggested internal fields:

```rust
struct RuntimeInfo {
    started_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    time_limit_seconds: Option<u64>,
    remaining_seconds: Option<u64>,
}
```

The frontend can update the visible countdown locally once it has an authoritative `ends_at`, while periodically refreshing from the scheduler to correct drift or changes.

---

## Expired Sessions

When a session disappears from the scheduler or reports a terminal state:

```text
Running
   ↓
Expired / Completed / Cancelled
```

Butler should:

1. close the SSH tunnel,
2. destroy the associated Code Server WebView,
3. remove it from the active session list,
4. retain local metadata long enough to support relaunch/reassociation later.

A project name and remote path should not disappear simply because the compute session expired.

---

## Remote Session Termination

The trash icon should map to the scheduler's cancellation operation.

For Slurm:

```bash
scancel <job_id>
```

Recommended UX:

```text
Kill Research Training?

This terminates the remote compute allocation and all
processes running inside it.

[Cancel]                    [Kill Session]
```

After successful cancellation:

1. mark the session `cancelling`,
2. stop/destroy the WebView,
3. close its SSH tunnel,
4. refresh scheduler state,
5. remove it from active sessions once confirmed.

Do not treat closing the WebView as killing the session.

---

## Connection State

Because the application may be used through a university VPN or cellular hotspot, connection status needs to be a first-class part of the UI.

Suggested global states:

```text
Connected
Connecting
VPN / SSH unreachable
Authentication required
Reconnecting
Offline
```

A temporary loss of SSH should not erase the local project/session metadata or destroy WebViews immediately.

Use a grace period and attempt reconnection before declaring the remote state unknown.

---

## Security Model

### Never persist

Do not persist:

- Code Server passwords,
- OOD session secrets,
- authentication cookies outside the WebView's normal session store,
- arbitrary SSH command history.

### Safe local persistence

Persist:

- friendly names,
- project names,
- remote folder paths,
- session UUIDs,
- scheduler job IDs,
- last-seen timestamps,
- UI ordering/preferences.

### Command surface

Frontend code should request explicit operations rather than supply raw commands.

Good:

```text
kill_session(job_id)
open_session(session_id)
list_sessions()
```

Avoid:

```text
run_remote_command("...")
```

This keeps remote command injection out of the UI boundary.

---

## MVP Scope

### Include

- university SSH configuration,
- active Code Server session discovery,
- scheduler correlation,
- session naming,
- project grouping,
- remote project paths,
- hardware pills,
- remaining-runtime pills,
- one-click session open,
- embedded Code Server WebView,
- automatic tunnel management,
- remote session termination,
- expired-session cleanup.

### Explicitly defer

- launching new Open OnDemand sessions,
- reproducing the OOD submission form,
- arbitrary scheduler job management,
- multi-cluster support,
- drag-and-drop organization,
- advanced notifications,
- session resource resizing,
- transparent session renewal.

This keeps v1 focused on the pain point: managing sessions after OOD launches them.

---

## Implementation Plan

### Phase 0 — Technical spike

Prove the riskiest path before committing to the UI.

Build a minimal Tauri app that can:

1. connect to the university through SSH,
2. identify one running OOD Code Server session,
3. read its `connection.yml`,
4. establish local port forwarding,
5. create a child WebView,
6. load Code Server inside it,
7. authenticate without manual credential lookup.

**Exit criterion:** a live remote Code Server editor is usable inside a Tauri window.

---

### Phase 1 — Cluster backend

Implement `ClusterService`.

Deliverables:

- SSH lifecycle management,
- persistent/multiplexed connection,
- structured remote command execution,
- Slurm session querying,
- OOD UUID correlation,
- `connection.yml` parsing,
- normalized `RemoteSession` model.

**Exit criterion:** `list_sessions()` returns all active Code Server sessions in one structured response without scanning hundreds of dead ports.

---

### Phase 2 — Local data model

Add SQLite and local session metadata.

Deliverables:

- `projects` table,
- `session_metadata` table,
- friendly names,
- remote paths,
- project association,
- sidebar ordering.

**Exit criterion:** metadata survives application restarts independently of OOD session lifetime.

---

### Phase 3 — Sidebar UI

Build the core application shell.

Deliverables:

- project groups,
- session cards,
- running-state indicator,
- hardware pills,
- runtime pill,
- selection state,
- empty/loading/offline states.

**Exit criterion:** active sessions are understandable without opening OOD.

---

### Phase 4 — Session WebViews

Implement editor embedding and switching.

Deliverables:

- local port allocation,
- tunnel registry,
- child WebView creation,
- WebView positioning,
- show/hide switching,
- small LRU cache for open editors.

**Exit criterion:** switching among multiple running sessions feels like changing IDE workspaces rather than changing browser tabs.

---

### Phase 5 — Authentication automation

Harden credential handling.

Deliverables:

- on-demand credential retrieval,
- no credential logging,
- automated Code Server authentication where supported,
- graceful fallback if the university changes its login behavior.

**Exit criterion:** the user never manually hunts for `connection.yml` or copies a generated password.

---

### Phase 6 — Session termination and expiry

Deliverables:

- trash-can action,
- confirmation dialog,
- `scancel` integration,
- cancellation progress state,
- WebView/tunnel teardown,
- automatic expired-session cleanup,
- runtime warning states.

**Exit criterion:** OOD is no longer needed for routine session management.

---

### Phase 7 — Polish

Potential improvements:

- keyboard shortcuts for session switching,
- search/filter sessions,
- reorder projects,
- drag sessions between projects,
- "Open in Browser" fallback,
- last-active timestamps,
- connection quality indicator,
- reconnection behavior,
- macOS menu-bar integration,
- notifications shortly before expiration.

---

## Suggested Domain Models

```rust
struct RemoteSession {
    ood_session_id: String,
    job_id: String,
    state: SessionState,
    host: Option<String>,
    port: Option<u16>,
    hardware: HardwareAllocation,
    runtime: RuntimeInfo,
}

enum SessionState {
    Pending,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Expired,
    Unknown,
}

struct SessionMetadata {
    ood_session_id: String,
    friendly_name: Option<String>,
    project_id: Option<i64>,
    remote_path: Option<String>,
}

struct SessionViewModel {
    remote: RemoteSession,
    metadata: SessionMetadata,
    local_port: Option<u16>,
    webview_label: Option<String>,
}
```

---

## Key Design Decisions

### 1. OOD launches; Butler manages

Do not reproduce OOD submission behavior in v1.

This keeps the initial application small and avoids coupling Butler to university-specific submission forms before the core manager is proven.

### 2. Scheduler is authoritative for active state

Do not discover active sessions by probing every historical `connection.yml`.

Slurm already knows which jobs are alive.

### 3. Local metadata is separate from remote state

Friendly names and projects belong to Butler, not OOD.

This makes them durable across short-lived compute allocations.

### 4. One native window, multiple WebViews

The sidebar remains a local Tauri UI; Code Server runs in child WebViews.

This is cleaner than iframes and allows multiple sessions to remain warm for fast switching.

### 5. Credentials are ephemeral

Retrieve remote credentials only when needed and never persist them.

### 6. Use existing OpenSSH initially

The system SSH client already solves university-specific authentication, configuration, key-agent integration, and ProxyJump behavior.

A native Rust SSH implementation can be considered later only if it provides a clear UX benefit.

---

## Future: Direct Session Launching

Once Butler is excellent at managing sessions, the next major feature can be launching them without visiting Open OnDemand.

There are two possible paths:

### Reproduce OOD submission

Butler could learn the university's OOD form/session submission pipeline and submit the same scheduler jobs.

This preserves compatibility with university-provided launch configuration but tightly couples Butler to that OOD deployment.

### Submit Code Server directly to Slurm

Butler could construct its own scheduler job and launch Code Server inside the allocation.

```text
Butler
   │
   ├── sbatch Code Server job
   │
   ▼
Slurm
   │
   ▼
compute node
   │
   ▼
Code Server
```

This gives Butler much more control but requires reproducing whatever modules, environment setup, security options, and cluster policy the university currently handles through OOD.

For that reason, direct launching should remain a later phase.

---

## Success Criteria

Butler v1 is successful when the normal workflow becomes:

```text
Launch session once in OOD
          ↓
Open Butler
          ↓
Research Training        [A100] [2d 11h]
CS 4501 Homework         [8 CPU] [18h 42m]
Scratch                  [4 CPU] [37m]
          ↓
click one
          ↓
Code Server appears immediately
```

The user should no longer need to:

- find OOD session UUIDs,
- inspect old session directories,
- manually locate `connection.yml`,
- copy host/port values,
- type SSH tunnel commands,
- copy Code Server passwords,
- keep many browser tabs organized,
- return to OOD to kill a running session.

OOD remains the session launcher; Butler becomes the day-to-day session workspace and manager.
