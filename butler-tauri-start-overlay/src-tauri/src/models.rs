use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub ood_session_id: String,
    pub job_id: String,
    pub friendly_name: String,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub remote_path: Option<String>,
    pub state: SessionState,
    pub hardware: HardwareAllocation,
    pub runtime: RuntimeInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Pending,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareAllocation {
    pub cpus: Option<u32>,
    pub memory_bytes: Option<u64>,
    pub gpus: Vec<GpuAllocation>,
    pub partition: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAllocation {
    pub model: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub remaining_seconds: Option<u64>,
    pub time_limit_seconds: Option<u64>,
}
