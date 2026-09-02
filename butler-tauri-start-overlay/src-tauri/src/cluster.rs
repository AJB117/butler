use std::sync::Mutex;

use crate::models::{
    GpuAllocation, HardwareAllocation, RuntimeInfo, Session, SessionState,
};

const GIB: u64 = 1024 * 1024 * 1024;

pub trait ClusterService: Send + Sync {
    fn list_sessions(&self) -> Result<Vec<Session>, String>;
    fn kill_session(&self, session_id: &str) -> Result<(), String>;
}

pub struct ClusterState {
    service: Box<dyn ClusterService>,
}

impl ClusterState {
    pub fn demo() -> Self {
        Self {
            service: Box::new(MockClusterService::new()),
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>, String> {
        self.service.list_sessions()
    }

    pub fn kill_session(&self, session_id: &str) -> Result<(), String> {
        self.service.kill_session(session_id)
    }
}

struct MockClusterService {
    sessions: Mutex<Vec<Session>>,
}

impl MockClusterService {
    fn new() -> Self {
        Self {
            sessions: Mutex::new(demo_sessions()),
        }
    }
}

impl ClusterService for MockClusterService {
    fn list_sessions(&self) -> Result<Vec<Session>, String> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "The local session list is unavailable".to_string())?;

        Ok(sessions.clone())
    }

    fn kill_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "The local session list is unavailable".to_string())?;

        let index = sessions
            .iter()
            .position(|session| session.ood_session_id == session_id)
            .ok_or_else(|| format!("Session {session_id} is no longer active"))?;

        sessions.remove(index);
        Ok(())
    }
}

fn demo_sessions() -> Vec<Session> {
    vec![
        Session {
            ood_session_id: "03a8efa7-da5f-436f-919f-617b948c1358".into(),
            job_id: "12345678".into(),
            friendly_name: "Training".into(),
            project_id: Some(1),
            project_name: Some("Research".into()),
            remote_path: Some("~/projects/research".into()),
            state: SessionState::Running,
            hardware: HardwareAllocation {
                cpus: Some(8),
                memory_bytes: Some(64 * GIB),
                gpus: vec![GpuAllocation {
                    model: Some("A100".into()),
                    count: 1,
                }],
                partition: Some("gpu".into()),
            },
            runtime: RuntimeInfo {
                remaining_seconds: Some(2 * 86_400 + 11 * 3_600),
                time_limit_seconds: Some(3 * 86_400),
            },
        },
        Session {
            ood_session_id: "5fa6ed27-f89d-46ca-a6b7-9e21d324c48f".into(),
            job_id: "12345681".into(),
            friendly_name: "Evaluation".into(),
            project_id: Some(1),
            project_name: Some("Research".into()),
            remote_path: Some("~/projects/research/evaluation".into()),
            state: SessionState::Pending,
            hardware: HardwareAllocation {
                cpus: Some(4),
                memory_bytes: Some(32 * GIB),
                gpus: vec![GpuAllocation {
                    model: None,
                    count: 1,
                }],
                partition: Some("gpu".into()),
            },
            runtime: RuntimeInfo {
                remaining_seconds: None,
                time_limit_seconds: Some(86_400),
            },
        },
        Session {
            ood_session_id: "f56f937c-a385-4d32-a583-c12767077017".into(),
            job_id: "12345692".into(),
            friendly_name: "Homework 3".into(),
            project_id: Some(2),
            project_name: Some("CS 4501".into()),
            remote_path: Some("~/courses/cs4501/homework-3".into()),
            state: SessionState::Running,
            hardware: HardwareAllocation {
                cpus: Some(8),
                memory_bytes: Some(32 * GIB),
                gpus: vec![],
                partition: Some("standard".into()),
            },
            runtime: RuntimeInfo {
                remaining_seconds: Some(18 * 3_600 + 42 * 60),
                time_limit_seconds: Some(86_400),
            },
        },
        Session {
            ood_session_id: "b8e40aaf-27b8-42c6-8918-4296d416dbab".into(),
            job_id: "12345703".into(),
            friendly_name: "Scratch".into(),
            project_id: Some(3),
            project_name: Some("Misc".into()),
            remote_path: Some("~/scratch".into()),
            state: SessionState::Running,
            hardware: HardwareAllocation {
                cpus: Some(4),
                memory_bytes: Some(16 * GIB),
                gpus: vec![],
                partition: Some("standard".into()),
            },
            runtime: RuntimeInfo {
                remaining_seconds: Some(37 * 60),
                time_limit_seconds: Some(4 * 3_600),
            },
        },
    ]
}
