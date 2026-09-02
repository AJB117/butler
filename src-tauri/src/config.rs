use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

const DEFAULT_OOD_DATA_ROOT: &str = "~/ondemand/data/sys/dashboard";

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ButlerConfig {
    pub ssh: SshConfig,
    pub ood: OodConfig,
    pub slurm: SlurmConfig,
}

impl Default for ButlerConfig {
    fn default() -> Self {
        Self {
            ssh: SshConfig::default(),
            ood: OodConfig::default(),
            slurm: SlurmConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct SshConfig {
    pub target: String,
    pub binary: String,
    pub config_file: Option<PathBuf>,
    pub port: Option<u16>,
    pub connect_timeout_seconds: u64,
    pub command_timeout_seconds: u64,
    pub control_persist_seconds: u64,
    pub multiplex: bool,
    pub control_path: Option<String>,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            target: String::new(),
            binary: "ssh".into(),
            config_file: None,
            port: None,
            connect_timeout_seconds: 10,
            command_timeout_seconds: 30,
            control_persist_seconds: 600,
            multiplex: cfg!(unix),
            control_path: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct OodConfig {
    pub data_root: String,
    pub app_tokens: Vec<String>,
}

impl Default for OodConfig {
    fn default() -> Self {
        Self {
            data_root: DEFAULT_OOD_DATA_ROOT.into(),
            app_tokens: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct SlurmConfig {
    pub squeue_binary: String,
    pub scancel_binary: String,
}

impl Default for SlurmConfig {
    fn default() -> Self {
        Self {
            squeue_binary: "squeue".into(),
            scancel_binary: "scancel".into(),
        }
    }
}

pub struct LoadedConfig {
    pub config: ButlerConfig,
    pub path: PathBuf,
}

pub fn default_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("config.json")
}

pub fn load(app_config_dir: &Path) -> Result<LoadedConfig, String> {
    let path = env::var_os("BUTLER_CONFIG")
        .map(PathBuf::from)
        .map(expand_local_home)
        .unwrap_or_else(|| default_path(app_config_dir));

    let mut config = if path.exists() {
        let contents = fs::read_to_string(&path).map_err(|error| {
            format!(
                "Could not read Butler config at {}: {error}",
                path.display()
            )
        })?;
        serde_json::from_str::<ButlerConfig>(&contents).map_err(|error| {
            format!(
                "Could not parse Butler config at {}: {error}",
                path.display()
            )
        })?
    } else {
        ButlerConfig::default()
    };

    apply_environment(&mut config)?;
    normalize(&mut config);
    validate(&config, &path)?;

    Ok(LoadedConfig { config, path })
}

fn apply_environment(config: &mut ButlerConfig) -> Result<(), String> {
    apply_string("BUTLER_SSH_TARGET", &mut config.ssh.target);
    apply_string("BUTLER_SSH_BINARY", &mut config.ssh.binary);
    apply_string("BUTLER_OOD_DATA_ROOT", &mut config.ood.data_root);
    apply_string("BUTLER_SQUEUE_BINARY", &mut config.slurm.squeue_binary);
    apply_string("BUTLER_SCANCEL_BINARY", &mut config.slurm.scancel_binary);

    if let Some(value) = env::var_os("BUTLER_SSH_CONFIG") {
        config.ssh.config_file = Some(expand_local_home(PathBuf::from(value)));
    }
    if let Ok(value) = env::var("BUTLER_SSH_CONTROL_PATH") {
        config.ssh.control_path = nonempty(value);
    }
    if let Ok(value) = env::var("BUTLER_SSH_PORT") {
        config.ssh.port = Some(
            value
                .parse::<u16>()
                .map_err(|_| "BUTLER_SSH_PORT must be a valid TCP port".to_string())?,
        );
    }
    if let Ok(value) = env::var("BUTLER_SSH_MULTIPLEX") {
        config.ssh.multiplex = parse_bool("BUTLER_SSH_MULTIPLEX", &value)?;
    }
    if let Ok(value) = env::var("BUTLER_OOD_APP_TOKENS") {
        config.ood.app_tokens = value
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect();
    }

    Ok(())
}

fn apply_string(name: &str, destination: &mut String) {
    if let Ok(value) = env::var(name) {
        *destination = value;
    }
}

fn parse_bool(name: &str, value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("{name} must be true or false")),
    }
}

fn normalize(config: &mut ButlerConfig) {
    config.ssh.target = config.ssh.target.trim().to_owned();
    config.ssh.binary = config.ssh.binary.trim().to_owned();
    config.ood.data_root = config.ood.data_root.trim().to_owned();
    config.slurm.squeue_binary = config.slurm.squeue_binary.trim().to_owned();
    config.slurm.scancel_binary = config.slurm.scancel_binary.trim().to_owned();
    config.ood.app_tokens = config
        .ood
        .app_tokens
        .iter()
        .map(|token| token.trim())
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect();

    if let Some(path) = config.ssh.config_file.take() {
        config.ssh.config_file = Some(expand_local_home(path));
    }
    config.ssh.control_path = config.ssh.control_path.take().and_then(nonempty);
}

fn validate(config: &ButlerConfig, path: &Path) -> Result<(), String> {
    if config.ssh.target.is_empty() {
        return Err(format!(
            "Butler is not configured. Copy config.example.json to {} and set ssh.target, or set BUTLER_SSH_TARGET.",
            path.display()
        ));
    }
    if config.ssh.target.starts_with('-') || contains_control(&config.ssh.target) {
        return Err("ssh.target is invalid".into());
    }
    if config.ssh.port == Some(0) {
        return Err("ssh.port cannot be 0".into());
    }
    if config.ssh.connect_timeout_seconds == 0 || config.ssh.command_timeout_seconds == 0 {
        return Err("SSH timeout values must be greater than 0".into());
    }
    if config.ssh.binary.is_empty()
        || config.slurm.squeue_binary.is_empty()
        || config.slurm.scancel_binary.is_empty()
    {
        return Err("Configured executable names cannot be empty".into());
    }
    if config.ood.data_root.is_empty() || contains_control(&config.ood.data_root) {
        return Err("ood.dataRoot is invalid".into());
    }
    if config
        .ood
        .app_tokens
        .iter()
        .any(|token| contains_control(token))
    {
        return Err("ood.appTokens contains an invalid token".into());
    }
    if config
        .ssh
        .control_path
        .as_deref()
        .is_some_and(contains_control)
    {
        return Err("ssh.controlPath is invalid".into());
    }

    Ok(())
}

fn contains_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn expand_local_home(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" || text.starts_with("~/") || text.starts_with("~\\") {
        if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
            if text == "~" {
                return PathBuf::from(home);
            }
            return PathBuf::from(home).join(&text[2..]);
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_standard_ood_and_slurm_layout() {
        let config = ButlerConfig::default();
        assert_eq!(config.ood.data_root, "~/ondemand/data/sys/dashboard");
        assert_eq!(config.slurm.squeue_binary, "squeue");
        assert_eq!(config.slurm.scancel_binary, "scancel");
    }

    #[test]
    fn validation_requires_an_ssh_target() {
        let config = ButlerConfig::default();
        assert!(validate(&config, Path::new("config.json")).is_err());
    }

    #[test]
    fn boolean_environment_values_are_strict() {
        assert!(parse_bool("TEST", "yes").unwrap());
        assert!(!parse_bool("TEST", "OFF").unwrap());
        assert!(parse_bool("TEST", "maybe").is_err());
    }
}
