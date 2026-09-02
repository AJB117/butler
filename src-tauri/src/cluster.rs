use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::Child,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Map, Value};

use crate::{
    config::{self, ButlerConfig},
    models::{
        BackendStatus, GpuAllocation, HardwareAllocation, OpenSessionResult, RuntimeInfo,
        Session, SessionState,
    },
    ssh::{SpawnedTunnel, SshClient},
};

const RECORD_SEPARATOR: u8 = 0x1e;
const FIELD_SEPARATOR: u8 = 0x1f;
const DISCOVERY_MARKER: &[u8] = b"BUTLER_DISCOVERY_V1\n";
const CONNECTION_MARKER: &[u8] = b"BUTLER_CONNECTION_V1\n";
const TUNNEL_READY_TIMEOUT: Duration = Duration::from_secs(5);

const DISCOVERY_SCRIPT: &str = r#"set -eu
root=$1
squeue_bin=$2

case "$root" in
  "~") root=$HOME ;;
  "~/"*) root="$HOME/${root#\~/}" ;;
esac

record_separator=$(printf '\036')
field_separator=$(printf '\037')
printf 'BUTLER_DISCOVERY_V1\n'
db_root="$root/batch_connect/db"

if [ -d "$db_root" ]; then
  for file in "$db_root"/*; do
    [ -f "$file" ] || continue
    [ -r "$file" ] || continue
    case "$file" in
      *.bak) continue ;;
    esac

    printf '%sO%s%s%s' "$record_separator" "$field_separator" "${file##*/}" "$field_separator"
    cat "$file"
  done
fi

user=${USER:-$(id -un)}
tmp="${TMPDIR:-/tmp}/butler-squeue-$$"
trap 'rm -f "$tmp"' EXIT HUP INT TERM

"$squeue_bin" \
  --noheader \
  --user="$user" \
  --states=PENDING,RUNNING,CONFIGURING,COMPLETING \
  --format='%i|%T|%N|%D|%C|%m|%L|%l|%P|%b' \
  > "$tmp"

while IFS= read -r line || [ -n "$line" ]; do
  printf '%sS%s%s' "$record_separator" "$field_separator" "$line"
done < "$tmp"

printf '%s' "$record_separator"
"#;

const CONNECTION_SCRIPT: &str = r#"set -eu
root=$1
cluster=$2
token=$3
session_id=$4

case "$root" in
  "~") root=$HOME ;;
  "~/"*) root="$HOME/${root#\~/}" ;;
esac

base="$root/batch_connect"

emit_if_readable() {
  if [ -f "$1" ] && [ -r "$1" ]; then
    printf 'BUTLER_CONNECTION_V1\n'
    cat "$1"
    exit 0
  fi
}

if [ -n "$cluster" ]; then
  emit_if_readable "$base/$cluster/$token/output/$session_id/connection.yml"
fi
emit_if_readable "$base/$token/output/$session_id/connection.yml"

found=$(find "$base" -type f -path "*/output/$session_id/connection.yml" -print -quit 2>/dev/null || true)
if [ -n "$found" ]; then
  printf 'BUTLER_CONNECTION_V1\n'
  cat "$found"
  exit 0
fi

printf 'connection.yml not found for OOD session %s\n' "$session_id" >&2
exit 4
"#;

#[derive(Clone)]
pub struct ClusterState {
    inner: Arc<ClusterStateInner>,
}

struct ClusterStateInner {
    backend: BackendState,
    config_path: PathBuf,
}

enum BackendState {
    Ready(OodSlurmBackend),
    Unconfigured(String),
}

impl ClusterState {
    pub fn load(app_config_dir: PathBuf) -> Self {
        let fallback_path = config::default_path(&app_config_dir);
        let inner = match config::load(&app_config_dir) {
            Ok(loaded) => ClusterStateInner {
                backend: BackendState::Ready(OodSlurmBackend::new(loaded.config)),
                config_path: loaded.path,
            },
            Err(message) => ClusterStateInner {
                backend: BackendState::Unconfigured(message),
                config_path: fallback_path,
            },
        };

        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn status(&self) -> BackendStatus {
        match &self.inner.backend {
            BackendState::Ready(backend) => backend.status(self.inner.config_path.clone()),
            BackendState::Unconfigured(message) => BackendStatus {
                configured: false,
                connected: false,
                config_path: self.inner.config_path.to_string_lossy().into_owned(),
                ssh_target: None,
                control_path: None,
                active_tunnels: 0,
                message: Some(message.clone()),
            },
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>, String> {
        self.backend()?.list_sessions()
    }

    pub fn open_session(&self, session_id: &str) -> Result<OpenSessionResult, String> {
        self.backend()?.open_session(session_id)
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        self.backend()?.close_session(session_id)
    }

    pub fn kill_session(&self, session_id: &str) -> Result<Vec<Session>, String> {
        self.backend()?.kill_session(session_id)
    }

    fn backend(&self) -> Result<&OodSlurmBackend, String> {
        match &self.inner.backend {
            BackendState::Ready(backend) => Ok(backend),
            BackendState::Unconfigured(message) => Err(message.clone()),
        }
    }
}

struct OodSlurmBackend {
    config: ButlerConfig,
    ssh: SshClient,
    sessions: RwLock<HashMap<String, CachedSession>>,
    tunnels: Mutex<HashMap<String, TunnelEntry>>,
    open_lock: Mutex<()>,
}

impl OodSlurmBackend {
    fn new(config: ButlerConfig) -> Self {
        let ssh = SshClient::new(config.ssh.clone());
        Self {
            config,
            ssh,
            sessions: RwLock::new(HashMap::new()),
            tunnels: Mutex::new(HashMap::new()),
            open_lock: Mutex::new(()),
        }
    }

    fn status(&self, config_path: PathBuf) -> BackendStatus {
        let active_tunnels = self
            .tunnels
            .lock()
            .map(|tunnels| tunnels.len())
            .unwrap_or(0);
        let connection = self.ssh.check_connection();

        BackendStatus {
            configured: true,
            connected: connection.is_ok(),
            config_path: config_path.to_string_lossy().into_owned(),
            ssh_target: Some(self.ssh.target().to_owned()),
            control_path: self.ssh.control_path().map(str::to_owned),
            active_tunnels,
            message: connection.err(),
        }
    }

    fn list_sessions(&self) -> Result<Vec<Session>, String> {
        let output = self.ssh.run_script(
            DISCOVERY_SCRIPT,
            &[
                self.config.ood.data_root.clone(),
                self.config.slurm.squeue_binary.clone(),
            ],
        )?;
        let snapshot = parse_discovery_output(&output)?;
        let mut sessions = Vec::new();
        let mut cache = HashMap::new();

        for ood in snapshot.ood_sessions {
            if ood.cache_completed || !is_code_server_session(&ood, &self.config.ood.app_tokens)
            {
                continue;
            }
            let Some(job) = find_job(&snapshot.jobs, &ood.job_id).cloned() else {
                continue;
            };

            sessions.push(build_session(&ood, &job));
            cache.insert(ood.id.clone(), CachedSession { ood, job });
        }

        sessions.sort_by(|left, right| {
            state_rank(left.state)
                .cmp(&state_rank(right.state))
                .then_with(|| left.friendly_name.cmp(&right.friendly_name))
        });

        let active_ids = cache.keys().cloned().collect::<HashSet<_>>();
        *self
            .sessions
            .write()
            .map_err(|_| "The session cache is unavailable".to_string())? = cache;
        self.remove_stale_tunnels(&active_ids)?;
        Ok(sessions)
    }

    fn open_session(&self, session_id: &str) -> Result<OpenSessionResult, String> {
        let _guard = self
            .open_lock
            .lock()
            .map_err(|_| "The tunnel manager is unavailable".to_string())?;
        let cached = self.cached_session(session_id)?;

        if cached.job.state != SessionState::Running {
            return Err(format!(
                "{} is not running yet; its scheduler state is {}",
                display_name(&cached.ood),
                state_label(cached.job.state)
            ));
        }

        validate_session_path_data(&cached.ood)?;
        let document = self.ssh.run_script(
            CONNECTION_SCRIPT,
            &[
                self.config.ood.data_root.clone(),
                cached.ood.cluster_id.clone(),
                cached.ood.token.clone(),
                cached.ood.id.clone(),
            ],
        )?;
        let document = marker_payload(&document, CONNECTION_MARKER, "connection.yml")?;
        let document = String::from_utf8(document.to_vec())
            .map_err(|_| "connection.yml is not valid UTF-8".to_string())?;
        let connection = parse_connection_document(&document)?;

        if let Some(local_port) = self.live_tunnel_port(session_id)? {
            return Ok(open_result(session_id, local_port, connection));
        }

        let local_port = allocate_local_port()?;
        let spawned = self
            .ssh
            .spawn_tunnel(local_port, &connection.host, connection.port)?;
        let tunnel = self.wait_for_tunnel(local_port, spawned)?;
        self.tunnels
            .lock()
            .map_err(|_| "The tunnel registry is unavailable".to_string())?
            .insert(session_id.to_owned(), tunnel);

        Ok(open_result(session_id, local_port, connection))
    }

    fn close_session(&self, session_id: &str) -> Result<(), String> {
        let tunnel = self
            .tunnels
            .lock()
            .map_err(|_| "The tunnel registry is unavailable".to_string())?
            .remove(session_id);
        drop(tunnel);
        Ok(())
    }

    fn kill_session(&self, session_id: &str) -> Result<Vec<Session>, String> {
        let cached = self.cached_session(session_id)?;
        validate_job_id(&cached.ood.job_id)?;
        self.ssh.run(
            &self.config.slurm.scancel_binary,
            std::slice::from_ref(&cached.ood.job_id),
        )?;
        self.close_session(session_id)?;
        self.list_sessions()
    }

    fn cached_session(&self, session_id: &str) -> Result<CachedSession, String> {
        if let Some(session) = self
            .sessions
            .read()
            .map_err(|_| "The session cache is unavailable".to_string())?
            .get(session_id)
            .cloned()
        {
            return Ok(session);
        }

        self.list_sessions()?;
        self.sessions
            .read()
            .map_err(|_| "The session cache is unavailable".to_string())?
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Session {session_id} is no longer active"))
    }

    fn live_tunnel_port(&self, session_id: &str) -> Result<Option<u16>, String> {
        let stale = {
            let mut tunnels = self
                .tunnels
                .lock()
                .map_err(|_| "The tunnel registry is unavailable".to_string())?;
            let live = match tunnels.get_mut(session_id) {
                Some(tunnel) => matches!(tunnel.child.try_wait(), Ok(None)),
                None => return Ok(None),
            };
            if live {
                return Ok(tunnels.get(session_id).map(|tunnel| tunnel.local_port));
            }
            tunnels.remove(session_id)
        };
        drop(stale);
        Ok(None)
    }

    fn wait_for_tunnel(
        &self,
        local_port: u16,
        spawned: SpawnedTunnel,
    ) -> Result<TunnelEntry, String> {
        let SpawnedTunnel {
            mut child,
            stderr_reader,
        } = spawned;
        let mut stderr_reader = Some(stderr_reader);
        let deadline = Instant::now() + TUNNEL_READY_TIMEOUT;
        let address = SocketAddr::from(([127, 0, 0, 1], local_port));

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stderr = join_reader(stderr_reader.take());
                    return Err(self.ssh.format_tunnel_failure(status, &stderr));
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = join_reader(stderr_reader.take());
                    return Err(format!("Could not inspect the SSH tunnel process: {error}"));
                }
            }

            if TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok() {
                return Ok(TunnelEntry {
                    child,
                    stderr_reader,
                    local_port,
                });
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = join_reader(stderr_reader.take());
                return Err(format!(
                    "SSH opened but the localhost tunnel did not become ready within {} seconds",
                    TUNNEL_READY_TIMEOUT.as_secs()
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn remove_stale_tunnels(&self, active_ids: &HashSet<String>) -> Result<(), String> {
        let stale = {
            let mut tunnels = self
                .tunnels
                .lock()
                .map_err(|_| "The tunnel registry is unavailable".to_string())?;
            let stale_ids = tunnels
                .keys()
                .filter(|session_id| !active_ids.contains(*session_id))
                .cloned()
                .collect::<Vec<_>>();
            stale_ids
                .into_iter()
                .filter_map(|session_id| tunnels.remove(&session_id))
                .collect::<Vec<_>>()
        };
        drop(stale);
        Ok(())
    }
}

#[derive(Clone)]
struct CachedSession {
    ood: OodSessionRecord,
    job: SlurmJob,
}

struct TunnelEntry {
    child: Child,
    stderr_reader: Option<thread::JoinHandle<Vec<u8>>>,
    local_port: u16,
}

impl Drop for TunnelEntry {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}

#[derive(Clone, Debug)]
struct OodSessionRecord {
    id: String,
    cluster_id: String,
    job_id: String,
    token: String,
    title: Option<String>,
    cache_completed: bool,
}

#[derive(Clone, Debug)]
struct SlurmJob {
    id: String,
    state: SessionState,
    nodes: Option<u32>,
    cpus: Option<u32>,
    minimum_memory: Option<String>,
    time_left: Option<String>,
    time_limit: Option<String>,
    partition: Option<String>,
    tres_per_node: Option<String>,
}

struct DiscoverySnapshot {
    ood_sessions: Vec<OodSessionRecord>,
    jobs: Vec<SlurmJob>,
}

struct ConnectionInfo {
    host: String,
    port: u16,
    password: Option<String>,
    protocol: String,
    path: String,
}

fn marker_payload<'a>(output: &'a [u8], marker: &[u8], label: &str) -> Result<&'a [u8], String> {
    output
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|position| &output[position + marker.len()..])
        .ok_or_else(|| format!("SSH {label} output did not contain Butler's protocol marker"))
}

fn parse_discovery_output(output: &[u8]) -> Result<DiscoverySnapshot, String> {
    let output = marker_payload(output, DISCOVERY_MARKER, "session discovery")?;
    let mut ood_sessions = Vec::new();
    let mut jobs = Vec::new();

    for record in output.split(|byte| *byte == RECORD_SEPARATOR) {
        if record.is_empty() {
            continue;
        }
        if let Some(payload) = record.strip_prefix(&[b'O', FIELD_SEPARATOR]) {
            let separator = payload
                .iter()
                .position(|byte| *byte == FIELD_SEPARATOR)
                .ok_or_else(|| "OOD discovery returned a malformed database record".to_string())?;
            let file = String::from_utf8_lossy(&payload[..separator]);
            match parse_ood_session_record(&file, &payload[separator + 1..]) {
                Ok(session) => ood_sessions.push(session),
                Err(error) => eprintln!("Butler ignored {error}"),
            }
        } else if let Some(payload) = record.strip_prefix(&[b'S', FIELD_SEPARATOR]) {
            let line = std::str::from_utf8(payload)
                .map_err(|_| "Slurm returned non-UTF-8 output".to_string())?;
            jobs.push(parse_slurm_job(line)?);
        } else {
            return Err("SSH discovery returned an unknown record type".into());
        }
    }

    Ok(DiscoverySnapshot { ood_sessions, jobs })
}

fn parse_ood_session_record(file: &str, json: &[u8]) -> Result<OodSessionRecord, String> {
    let value: Value = serde_json::from_slice(json)
        .map_err(|error| format!("malformed OOD database file {file}: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("non-object OOD database file {file}"))?;
    let id = object_text(object, "id")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| file.to_owned());

    Ok(OodSessionRecord {
        id,
        cluster_id: object_text(object, "cluster_id").unwrap_or_default(),
        job_id: required_object_text(object, "job_id", file)?,
        token: required_object_text(object, "token", file)?,
        title: object_text(object, "title").filter(|value| !value.trim().is_empty()),
        cache_completed: object
            .get("cache_completed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn required_object_text(
    object: &Map<String, Value>,
    key: &str,
    file: &str,
) -> Result<String, String> {
    object_text(object, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("OOD database file {file} has no {key}"))
}

fn object_text(object: &Map<String, Value>, key: &str) -> Option<String> {
    match object.get(key)? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn parse_slurm_job(line: &str) -> Result<SlurmJob, String> {
    let fields = line.splitn(10, '|').map(str::trim).collect::<Vec<_>>();
    if fields.len() != 10 {
        return Err(format!(
            "Slurm returned an unexpected squeue row with {} fields",
            fields.len()
        ));
    }
    if fields[0].is_empty() {
        return Err("Slurm returned a job with no ID".into());
    }

    Ok(SlurmJob {
        id: fields[0].to_owned(),
        state: parse_session_state(fields[1]),
        nodes: parse_optional_u32(fields[3]),
        cpus: parse_optional_u32(fields[4]),
        minimum_memory: optional_slurm_value(fields[5]),
        time_left: optional_slurm_value(fields[6]),
        time_limit: optional_slurm_value(fields[7]),
        partition: optional_slurm_value(fields[8])
            .map(|value| value.trim_end_matches('*').to_owned()),
        tres_per_node: optional_slurm_value(fields[9]),
    })
}

fn build_session(ood: &OodSessionRecord, job: &SlurmJob) -> Session {
    Session {
        ood_session_id: ood.id.clone(),
        job_id: job.id.clone(),
        friendly_name: display_name(ood),
        project_id: None,
        project_name: None,
        remote_path: None,
        state: job.state,
        hardware: HardwareAllocation {
            cpus: job.cpus,
            memory_bytes: job
                .minimum_memory
                .as_deref()
                .and_then(|memory| parse_slurm_memory(memory, job.cpus, job.nodes)),
            gpus: job
                .tres_per_node
                .as_deref()
                .map(parse_gpu_allocations)
                .unwrap_or_default(),
            partition: job.partition.clone(),
        },
        runtime: RuntimeInfo {
            remaining_seconds: job.time_left.as_deref().and_then(parse_slurm_duration),
            time_limit_seconds: job.time_limit.as_deref().and_then(parse_slurm_duration),
        },
    }
}

fn find_job<'a>(jobs: &'a [SlurmJob], job_id: &str) -> Option<&'a SlurmJob> {
    jobs.iter().find(|job| job.id == job_id).or_else(|| {
        let expected = canonical_job_id(job_id);
        jobs.iter()
            .find(|job| canonical_job_id(&job.id) == expected)
    })
}

fn canonical_job_id(job_id: &str) -> &str {
    let job_id = job_id.split(';').next().unwrap_or(job_id).trim();
    job_id
        .strip_suffix(".batch")
        .or_else(|| job_id.strip_suffix(".extern"))
        .unwrap_or(job_id)
}

fn is_code_server_session(record: &OodSessionRecord, app_tokens: &[String]) -> bool {
    if !app_tokens.is_empty() {
        return app_tokens.iter().any(|token| token == &record.token);
    }
    let title = record.title.as_deref().unwrap_or_default();
    let text = format!("{} {title}", record.token).to_ascii_lowercase();
    ["vscode", "code-server", "code_server", "code server", "codeserver"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn display_name(record: &OodSessionRecord) -> String {
    let base = record
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| humanize_token(&record.token));
    let suffix = record.id.chars().take(8).collect::<String>();
    if suffix.is_empty() {
        base
    } else {
        format!("{base} · {suffix}")
    }
}

fn humanize_token(token: &str) -> String {
    let name = token
        .rsplit('/')
        .next()
        .unwrap_or(token)
        .trim_start_matches("bc_");
    let words = name
        .split(|character| matches!(character, '_' | '-'))
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => {
                    let first = first.to_uppercase().collect::<String>();
                    format!("{first}{}", characters.as_str().to_ascii_lowercase())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        "Code Server".into()
    } else {
        words.join(" ")
    }
}

fn parse_session_state(value: &str) -> SessionState {
    match value.trim().to_ascii_uppercase().as_str() {
        "PD" | "PENDING" | "CF" | "CONFIGURING" => SessionState::Pending,
        "R" | "RUNNING" => SessionState::Running,
        "CG" | "COMPLETING" => SessionState::Cancelling,
        _ => SessionState::Unknown,
    }
}

fn state_rank(state: SessionState) -> u8 {
    match state {
        SessionState::Running => 0,
        SessionState::Pending => 1,
        SessionState::Cancelling => 2,
        SessionState::Unknown => 3,
        SessionState::Completed | SessionState::Cancelled | SessionState::Expired => 4,
    }
}

fn state_label(state: SessionState) -> &'static str {
    match state {
        SessionState::Pending => "pending",
        SessionState::Running => "running",
        SessionState::Cancelling => "completing",
        SessionState::Completed => "completed",
        SessionState::Cancelled => "cancelled",
        SessionState::Expired => "expired",
        SessionState::Unknown => "unknown",
    }
}

fn optional_slurm_value(value: &str) -> Option<String> {
    let value = value.trim();
    let upper = value.to_ascii_uppercase();
    if value.is_empty()
        || matches!(
            upper.as_str(),
            "N/A" | "NONE" | "(NULL)" | "NOT_SET" | "INVALID" | "UNLIMITED"
        )
    {
        None
    } else {
        Some(value.to_owned())
    }
}

fn parse_optional_u32(value: &str) -> Option<u32> {
    optional_slurm_value(value)?.parse().ok()
}

fn parse_slurm_duration(value: &str) -> Option<u64> {
    let value = value.trim();
    let upper = value.to_ascii_uppercase();
    if value.is_empty()
        || matches!(
            upper.as_str(),
            "N/A" | "NONE" | "NOT_SET" | "INVALID" | "UNLIMITED" | "PARTITION_LIMIT"
        )
    {
        return None;
    }

    let (days, clock) = match value.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, value),
    };
    let clock = clock.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds): (u64, u64, u64) = match clock.as_slice() {
        [minutes, seconds] => (0, minutes.parse().ok()?, seconds.parse().ok()?),
        [hours, minutes, seconds] => (
            hours.parse().ok()?,
            minutes.parse().ok()?,
            seconds.parse().ok()?,
        ),
        [minutes] => (0, minutes.parse().ok()?, 0),
        _ => return None,
    };

    days.checked_mul(86_400)?
        .checked_add(hours.checked_mul(3_600)?)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(seconds)
}

fn parse_slurm_memory(value: &str, cpus: Option<u32>, nodes: Option<u32>) -> Option<u64> {
    let mut value = value.trim();
    if value.is_empty() {
        return None;
    }

    let scope = value
        .chars()
        .last()
        .filter(|character| matches!(character.to_ascii_lowercase(), 'c' | 'n'));
    if scope.is_some() {
        value = &value[..value.len() - 1];
    }
    let unit = value
        .chars()
        .last()
        .filter(|character| character.is_ascii_alphabetic())
        .map(|character| character.to_ascii_uppercase())
        .unwrap_or('M');
    if value
        .chars()
        .last()
        .is_some_and(|character| character.is_ascii_alphabetic())
    {
        value = &value[..value.len() - 1];
    }

    let number = value.parse::<f64>().ok()?;
    if !number.is_finite() || number <= 0.0 {
        return None;
    }
    let unit_multiplier = match unit {
        'K' => 1024_f64,
        'M' => 1024_f64.powi(2),
        'G' => 1024_f64.powi(3),
        'T' => 1024_f64.powi(4),
        'P' => 1024_f64.powi(5),
        _ => return None,
    };
    let scope_multiplier = match scope.map(|scope| scope.to_ascii_lowercase()) {
        Some('c') => cpus.unwrap_or(1) as f64,
        Some('n') => nodes.unwrap_or(1) as f64,
        _ => 1.0,
    };
    let bytes = number * unit_multiplier * scope_multiplier;
    (bytes <= u64::MAX as f64).then_some(bytes.round() as u64)
}

fn parse_gpu_allocations(value: &str) -> Vec<GpuAllocation> {
    let mut allocations = BTreeMap::<Option<String>, u32>::new();

    for raw in value.split(',') {
        let lower = raw.to_ascii_lowercase();
        let Some(position) = lower.find("gpu") else {
            continue;
        };
        let remainder = raw[position + 3..]
            .trim()
            .trim_start_matches(|character| matches!(character, '/' | ':' | '='));
        let parts = remainder
            .split(|character| matches!(character, '/' | ':' | '='))
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();

        let (model, count) = match parts.as_slice() {
            [] => (None, 1),
            [count] if leading_u32(count).is_some() => (None, leading_u32(count).unwrap()),
            [model] => (Some(normalize_gpu_model(model)), 1),
            _ => {
                let count = parts.last().and_then(|part| leading_u32(part)).unwrap_or(1);
                let model = parts
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|part| leading_u32(part).is_none())
                    .map(|model| normalize_gpu_model(model));
                (model, count)
            }
        };
        allocations
            .entry(model)
            .and_modify(|total| *total = total.saturating_add(count))
            .or_insert(count);
    }

    allocations
        .into_iter()
        .map(|(model, count)| GpuAllocation { model, count })
        .collect()
}

fn leading_u32(value: &str) -> Option<u32> {
    let digits = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn normalize_gpu_model(model: &str) -> String {
    model
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '-')
        .replace('_', "-")
        .to_ascii_uppercase()
}

fn parse_connection_document(document: &str) -> Result<ConnectionInfo, String> {
    let values = parse_flat_yaml(document)?;
    let host = first_value(
        &values,
        &["host", "hostname", "code_server_host", "code-server-host"],
    )
    .ok_or_else(|| "connection.yml has no host".to_string())?;
    validate_connection_host(&host)?;

    let port = first_value(
        &values,
        &["port", "code_server_port", "code-server-port", "server_port"],
    )
    .ok_or_else(|| "connection.yml has no port".to_string())?
    .parse::<u16>()
    .map_err(|_| "connection.yml contains an invalid port".to_string())?;
    if port == 0 {
        return Err("connection.yml contains port 0".into());
    }

    let password = first_value(
        &values,
        &[
            "password",
            "code_server_password",
            "code-server-password",
            "connection_password",
        ],
    )
    .filter(|password| !password.is_empty());
    let protocol = first_value(&values, &["protocol", "scheme"])
        .unwrap_or_else(|| "http".into())
        .to_ascii_lowercase();
    if protocol != "http" && protocol != "https" {
        return Err(format!(
            "connection.yml uses unsupported protocol {protocol}"
        ));
    }
    let path = first_value(
        &values,
        &["path", "url_path", "base_path", "connection_path", "url"],
    )
    .map(|path| normalize_url_path(&path))
    .transpose()?
    .unwrap_or_else(|| "/".into());

    Ok(ConnectionInfo {
        host,
        port,
        password,
        protocol,
        path,
    })
}

fn parse_flat_yaml(document: &str) -> Result<HashMap<String, String>, String> {
    let mut values = HashMap::new();
    for (index, line) in document.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || matches!(line, "---" | "...") {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key
            .trim()
            .trim_matches(|character| matches!(character, '\'' | '"'))
            .to_ascii_lowercase();
        if !key.is_empty() {
            values.insert(key, decode_yaml_scalar(value, index + 1)?);
        }
    }
    Ok(values)
}

fn decode_yaml_scalar(raw: &str, line_number: usize) -> Result<String, String> {
    let value = strip_yaml_comment(raw).trim();
    if value.is_empty() || matches!(value.to_ascii_lowercase().as_str(), "null" | "~") {
        return Ok(String::new());
    }
    if value.starts_with('\'') {
        if value.len() < 2 || !value.ends_with('\'') {
            return Err(format!(
                "connection.yml line {line_number} has an unterminated single-quoted value"
            ));
        }
        return Ok(value[1..value.len() - 1].replace("''", "'"));
    }
    if value.starts_with('"') {
        return serde_json::from_str::<String>(value).map_err(|error| {
            format!(
                "connection.yml line {line_number} has an invalid double-quoted value: {error}"
            )
        });
    }
    Ok(value.to_owned())
}

fn strip_yaml_comment(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' if in_double => {
                index = (index + 2).min(bytes.len());
                continue;
            }
            b'\'' if !in_double => {
                if in_single && bytes.get(index + 1) == Some(&b'\'') {
                    index += 2;
                    continue;
                }
                in_single = !in_single;
            }
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single
                && !in_double
                && (index == 0 || bytes[index - 1].is_ascii_whitespace()) =>
            {
                return &value[..index];
            }
            _ => {}
        }
        index += 1;
    }
    value
}

fn first_value(values: &HashMap<String, String>, aliases: &[&str]) -> Option<String> {
    aliases
        .iter()
        .find_map(|alias| values.get(*alias))
        .cloned()
}

fn validate_connection_host(host: &str) -> Result<(), String> {
    if host.is_empty()
        || !host.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_' | ':' | '%')
        })
    {
        return Err("connection.yml contains an invalid host".into());
    }
    Ok(())
}

fn normalize_url_path(path: &str) -> Result<String, String> {
    if path
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err("connection.yml contains an invalid URL path".into());
    }
    let path = if let Some((_, authority_and_path)) = path.split_once("://") {
        authority_and_path
            .find('/')
            .map(|index| &authority_and_path[index..])
            .unwrap_or("/")
    } else {
        path
    };
    if path.is_empty() {
        Ok("/".into())
    } else if path.starts_with('/') {
        Ok(path.to_owned())
    } else {
        Ok(format!("/{path}"))
    }
}

fn validate_session_path_data(record: &OodSessionRecord) -> Result<(), String> {
    if record.id.is_empty()
        || !record.id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err("The OOD session ID cannot be used as a path component".into());
    }
    validate_relative_ood_path("app token", &record.token)?;
    if !record.cluster_id.is_empty() {
        validate_relative_ood_path("cluster ID", &record.cluster_id)?;
    }
    Ok(())
}

fn validate_relative_ood_path(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "..")
    {
        return Err(format!("The OOD {name} is not a safe relative path"));
    }
    Ok(())
}

fn validate_job_id(job_id: &str) -> Result<(), String> {
    if job_id.is_empty()
        || !job_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '+')
        })
    {
        return Err("The scheduler job ID is invalid".into());
    }
    Ok(())
}

fn allocate_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("Could not reserve a localhost port: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("Could not inspect the reserved localhost port: {error}"))
}

fn open_result(
    session_id: &str,
    local_port: u16,
    connection: ConnectionInfo,
) -> OpenSessionResult {
    OpenSessionResult {
        session_id: session_id.to_owned(),
        local_port,
        url: format!(
            "{}://127.0.0.1:{}{}",
            connection.protocol, local_port, connection.path
        ),
        remote_host: connection.host,
        remote_port: connection.port,
        password: connection.password,
    }
}

fn join_reader(reader: Option<thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_protocol_parses_ood_and_slurm_records() {
        let output = concat!(
            "BUTLER_DISCOVERY_V1\n",
            "\u{1e}O\u{1f}abc\u{1f}",
            r#"{"id":"abc","cluster_id":"rivanna","job_id":"123","token":"sys/bc_code_server","title":"Code Server","cache_completed":false}"#,
            "\u{1e}S\u{1f}123|RUNNING|udc-a1|1|8|64G|2-11:00:00|3-00:00:00|gpu|gres/gpu:a100:1",
            "\u{1e}"
        );
        let snapshot = parse_discovery_output(output.as_bytes()).unwrap();
        assert_eq!(snapshot.ood_sessions.len(), 1);
        assert_eq!(snapshot.jobs.len(), 1);
        assert_eq!(snapshot.jobs[0].state, SessionState::Running);
    }

    #[test]
    fn parses_slurm_resource_formats() {
        assert_eq!(parse_slurm_duration("2-11:04:03"), Some(212_643));
        assert_eq!(
            parse_slurm_memory("4Gc", Some(8), Some(1)),
            Some(32 * 1024 * 1024 * 1024)
        );
        let gpus = parse_gpu_allocations("gres/gpu:a100:2");
        assert_eq!(gpus[0].model.as_deref(), Some("A100"));
        assert_eq!(gpus[0].count, 2);
    }

    #[test]
    fn parses_connection_yaml_with_quoted_hash() {
        let connection = parse_connection_document(
            "host: udc-a1\nport: 3450\npassword: 'secret # value'\npath: /code/\n",
        )
        .unwrap();
        assert_eq!(connection.host, "udc-a1");
        assert_eq!(connection.port, 3450);
        assert_eq!(connection.password.as_deref(), Some("secret # value"));
        assert_eq!(connection.path, "/code/");
    }
}
