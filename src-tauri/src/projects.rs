use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::models::{Project, ProjectSnapshot};

const DEFAULT_PROJECT_ID: i64 = 0;
const PROJECTS_FILE_NAME: &str = "projects.json";

#[derive(Clone)]
pub struct ProjectState {
    inner: Arc<ProjectStateInner>,
}

struct ProjectStateInner {
    path: PathBuf,
    data: Mutex<ProjectData>,
}

#[derive(Clone, Serialize, Deserialize)]
struct ProjectData {
    #[serde(default = "default_next_project_id")]
    next_project_id: i64,
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    assignments: HashMap<String, i64>,
}

impl Default for ProjectData {
    fn default() -> Self {
        Self {
            next_project_id: 1,
            projects: vec![default_project()],
            assignments: HashMap::new(),
        }
    }
}

impl ProjectState {
    pub fn load(app_config_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(app_config_dir).map_err(|error| {
            format!(
                "Could not create Butler's application data directory {}: {error}",
                app_config_dir.display()
            )
        })?;

        let path = app_config_dir.join(PROJECTS_FILE_NAME);
        let mut data = match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents).map_err(|error| {
                format!(
                    "Could not parse Butler project metadata at {}: {error}",
                    path.display()
                )
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ProjectData::default(),
            Err(error) => {
                return Err(format!(
                    "Could not read Butler project metadata at {}: {error}",
                    path.display()
                ))
            }
        };

        repair_loaded_data(&mut data);
        save(&path, &data)?;

        Ok(Self {
            inner: Arc::new(ProjectStateInner {
                path,
                data: Mutex::new(data),
            }),
        })
    }

    pub fn snapshot(&self) -> Result<ProjectSnapshot, String> {
        let data = self.data()?;
        Ok(snapshot(&data))
    }

    pub fn create_project(
        &self,
        name: String,
        remote_path: Option<String>,
    ) -> Result<ProjectSnapshot, String> {
        let name = clean_name(&name)?;
        let remote_path = clean_path(remote_path)?;
        let mut data = self.data()?;
        ensure_unique_name(&data.projects, &name, None)?;

        let id = data.next_project_id.max(1);
        data.next_project_id = id.saturating_add(1);
        let sort_order = data
            .projects
            .iter()
            .map(|project| project.sort_order)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        data.projects.push(Project {
            id,
            name,
            remote_path,
            sort_order,
            is_default: false,
        });
        save(&self.inner.path, &data)?;
        Ok(snapshot(&data))
    }

    pub fn update_project(
        &self,
        project_id: i64,
        name: String,
        remote_path: Option<String>,
    ) -> Result<ProjectSnapshot, String> {
        let name = clean_name(&name)?;
        let remote_path = clean_path(remote_path)?;
        let mut data = self.data()?;
        ensure_unique_name(&data.projects, &name, Some(project_id))?;

        let project = data
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .ok_or_else(|| format!("Project {project_id} no longer exists"))?;
        project.name = name;
        project.remote_path = remote_path;
        save(&self.inner.path, &data)?;
        Ok(snapshot(&data))
    }

    pub fn delete_project(&self, project_id: i64) -> Result<ProjectSnapshot, String> {
        if project_id == DEFAULT_PROJECT_ID {
            return Err("The default project cannot be deleted".into());
        }

        let mut data = self.data()?;
        let project_count = data.projects.len();
        data.projects.retain(|project| project.id != project_id);
        if data.projects.len() == project_count {
            return Err(format!("Project {project_id} no longer exists"));
        }
        data.assignments
            .retain(|_, assigned_project_id| *assigned_project_id != project_id);
        save(&self.inner.path, &data)?;
        Ok(snapshot(&data))
    }

    pub fn assign_session(
        &self,
        session_id: String,
        project_id: i64,
    ) -> Result<ProjectSnapshot, String> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return Err("The session ID is invalid".into());
        }

        let mut data = self.data()?;
        if !data.projects.iter().any(|project| project.id == project_id) {
            return Err(format!("Project {project_id} no longer exists"));
        }
        if project_id == DEFAULT_PROJECT_ID {
            data.assignments.remove(session_id);
        } else {
            data.assignments.insert(session_id.to_owned(), project_id);
        }
        save(&self.inner.path, &data)?;
        Ok(snapshot(&data))
    }

    fn data(&self) -> Result<std::sync::MutexGuard<'_, ProjectData>, String> {
        self.inner
            .data
            .lock()
            .map_err(|_| "The project metadata store is unavailable".to_string())
    }
}

fn default_next_project_id() -> i64 {
    1
}

fn default_project() -> Project {
    Project {
        id: DEFAULT_PROJECT_ID,
        name: "Unassigned".into(),
        remote_path: None,
        sort_order: 0,
        is_default: true,
    }
}

fn repair_loaded_data(data: &mut ProjectData) {
    if !data
        .projects
        .iter()
        .any(|project| project.id == DEFAULT_PROJECT_ID)
    {
        data.projects.push(default_project());
    }

    for project in &mut data.projects {
        project.is_default = project.id == DEFAULT_PROJECT_ID;
    }
    data.projects.sort_by_key(|project| (project.sort_order, project.id));

    let project_ids = data
        .projects
        .iter()
        .map(|project| project.id)
        .collect::<HashSet<_>>();
    data.assignments.retain(|session_id, project_id| {
        !session_id.trim().is_empty()
            && *project_id != DEFAULT_PROJECT_ID
            && project_ids.contains(project_id)
    });

    let next_id = data
        .projects
        .iter()
        .map(|project| project.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    data.next_project_id = data.next_project_id.max(next_id).max(1);
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Project name cannot be empty".into());
    }
    if name.chars().count() > 80 {
        return Err("Project names must be 80 characters or fewer".into());
    }
    Ok(name.to_owned())
}

fn clean_path(path: Option<String>) -> Result<Option<String>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let path = path.trim();
    if path.is_empty() {
        return Ok(None);
    }
    if path.chars().count() > 1_024 {
        return Err("Remote folders must be 1,024 characters or fewer".into());
    }
    Ok(Some(path.to_owned()))
}

fn ensure_unique_name(
    projects: &[Project],
    name: &str,
    except_project_id: Option<i64>,
) -> Result<(), String> {
    if projects.iter().any(|project| {
        Some(project.id) != except_project_id && project.name.eq_ignore_ascii_case(name)
    }) {
        return Err(format!("A project named “{name}” already exists"));
    }
    Ok(())
}

fn snapshot(data: &ProjectData) -> ProjectSnapshot {
    ProjectSnapshot {
        projects: data.projects.clone(),
        assignments: data.assignments.clone(),
    }
}

fn save(path: &Path, data: &ProjectData) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(data)
        .map_err(|error| format!("Could not serialize Butler project metadata: {error}"))?;
    fs::write(path, contents).map_err(|error| {
        format!(
            "Could not save Butler project metadata at {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn projects_and_assignments_survive_reload() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "butler-projects-{}-{unique}",
            std::process::id()
        ));

        let state = ProjectState::load(&directory).unwrap();
        let snapshot = state
            .create_project("Research".into(), Some("~/research".into()))
            .unwrap();
        let project_id = snapshot
            .projects
            .iter()
            .find(|project| project.name == "Research")
            .unwrap()
            .id;
        state
            .assign_session("session-a".into(), project_id)
            .unwrap();

        let reloaded = ProjectState::load(&directory).unwrap().snapshot().unwrap();
        assert_eq!(reloaded.assignments.get("session-a"), Some(&project_id));
        assert!(reloaded.projects.iter().any(|project| project.is_default));
        assert!(reloaded
            .projects
            .iter()
            .any(|project| project.name == "Research"));

        let _ = fs::remove_dir_all(directory);
    }
}
