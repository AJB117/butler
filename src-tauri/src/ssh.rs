use std::{
    io::{Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::config::SshConfig;

pub struct SshClient {
    config: SshConfig,
    control_path: Option<String>,
}

pub struct SpawnedTunnel {
    pub child: Child,
    pub stderr_reader: thread::JoinHandle<Vec<u8>>,
}

impl SshClient {
    pub fn new(config: SshConfig) -> Self {
        let control_path = if config.multiplex {
            config
                .control_path
                .clone()
                .or_else(|| Some(default_control_path()))
        } else {
            None
        };

        Self {
            config,
            control_path,
        }
    }

    pub fn target(&self) -> &str {
        &self.config.target
    }

    pub fn control_path(&self) -> Option<&str> {
        self.control_path.as_deref()
    }

    pub fn run(&self, program: &str, args: &[String]) -> Result<Vec<u8>, String> {
        let mut remote_parts = Vec::with_capacity(args.len() + 1);
        remote_parts.push(program.to_owned());
        remote_parts.extend(args.iter().cloned());
        let remote_command = shell_join(&remote_parts)?;

        self.execute(remote_command, None)
    }

    pub fn run_script(&self, script: &str, args: &[String]) -> Result<Vec<u8>, String> {
        let mut remote_parts = vec!["sh".to_owned(), "-s".to_owned(), "--".to_owned()];
        remote_parts.extend(args.iter().cloned());
        let remote_command = shell_join(&remote_parts)?;

        self.execute(remote_command, Some(script.as_bytes()))
    }

    pub fn check_connection(&self) -> Result<(), String> {
        self.run("true", &[]).map(|_| ())
    }

    pub fn spawn_tunnel(
        &self,
        local_port: u16,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<SpawnedTunnel, String> {
        let host = if remote_host.contains(':') {
            format!("[{remote_host}]")
        } else {
            remote_host.to_owned()
        };
        let forward = format!("127.0.0.1:{local_port}:{host}:{remote_port}");

        let mut command = self.base_command();
        command
            .arg("-N")
            .arg("-o")
            .arg("ExitOnForwardFailure=yes")
            .arg("-L")
            .arg(forward)
            .arg(&self.config.target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            format!(
                "Could not start {} for the Code Server tunnel: {error}",
                self.config.binary
            )
        })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Could not capture SSH tunnel errors".to_string())?;
        let stderr_reader = thread::spawn(move || read_all(stderr));

        Ok(SpawnedTunnel {
            child,
            stderr_reader,
        })
    }

    pub fn format_tunnel_failure(&self, status: ExitStatus, stderr: &[u8]) -> String {
        format_process_failure("SSH tunnel", status, stderr)
    }

    fn execute(&self, remote_command: String, stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let mut command = self.base_command();
        command
            .arg(&self.config.target)
            .arg(remote_command)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = run_with_timeout(
            command,
            stdin,
            Duration::from_secs(self.config.command_timeout_seconds),
        )?;

        if output.status.success() {
            return Ok(output.stdout);
        }

        Err(format_process_failure(
            "SSH command",
            output.status,
            &output.stderr,
        ))
    }

    fn base_command(&self) -> Command {
        let mut command = Command::new(&self.config.binary);

        if let Some(config_file) = &self.config.config_file {
            command.arg("-F").arg(config_file);
        }

        if let Some(port) = self.config.port {
            command.arg("-p").arg(port.to_string());
        }

        command
            .arg("-T")
            .arg("-o")
            .arg(format!(
                "ConnectTimeout={}",
                self.config.connect_timeout_seconds
            ))
            .arg("-o")
            .arg("ServerAliveInterval=15")
            .arg("-o")
            .arg("ServerAliveCountMax=2")
            .arg("-o")
            .arg("LogLevel=ERROR");

        if let Some(control_path) = &self.control_path {
            command
                .arg("-o")
                .arg("ControlMaster=auto")
                .arg("-o")
                .arg(format!(
                    "ControlPersist={}",
                    self.config.control_persist_seconds
                ))
                .arg("-o")
                .arg(format!("ControlPath={control_path}"));
        }

        command
    }
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_with_timeout(
    mut command: Command,
    stdin_data: Option<&[u8]>,
    timeout: Duration,
) -> Result<ProcessOutput, String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start {program}: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Could not capture {program} stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Could not capture {program} stderr"))?;
    let stdout_reader = thread::spawn(move || read_all(stdout));
    let stderr_reader = thread::spawn(move || read_all(stderr));

    if let Some(data) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(data);
        }
    }

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!(
                    "{program} did not finish within {} seconds",
                    timeout.as_secs()
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("Could not wait for {program}: {error}"));
            }
        }
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| format!("Could not collect {program} stdout"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| format!("Could not collect {program} stderr"))?;

    Ok(ProcessOutput {
        status,
        stdout,
        stderr,
    })
}

fn read_all(mut reader: impl Read) -> Vec<u8> {
    let mut output = Vec::new();
    let _ = reader.read_to_end(&mut output);
    output
}

fn shell_join(parts: &[String]) -> Result<String, String> {
    parts
        .iter()
        .map(|part| shell_quote(part))
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(" "))
}

fn shell_quote(value: &str) -> Result<String, String> {
    if value
        .chars()
        .any(|character| matches!(character, '\0' | '\n' | '\r'))
    {
        return Err("A remote command argument contains an unsupported control character".into());
    }

    if !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | '=' | ','
                )
        })
    {
        return Ok(value.to_owned());
    }

    Ok(format!("'{}'", value.replace('\'', "'\"'\"'")))
}

fn default_control_path() -> String {
    std::env::temp_dir()
        .join("butler-%C.sock")
        .to_string_lossy()
        .into_owned()
}

fn format_process_failure(label: &str, status: ExitStatus, stderr: &[u8]) -> String {
    let detail = clean_error(stderr);
    let lower = detail.to_ascii_lowercase();

    let hint = if lower.contains("permission denied") || lower.contains("authentication failed") {
        " SSH authentication failed. Verify your key or agent and complete any required MFA in a terminal once."
    } else if lower.contains("could not resolve hostname") {
        " The SSH host could not be resolved; check the VPN and ssh.target."
    } else if lower.contains("connection timed out") || lower.contains("operation timed out") {
        " The SSH connection timed out; check the VPN or hotspot connection."
    } else if lower.contains("no route to host") || lower.contains("network is unreachable") {
        " The cluster is unreachable; check the university VPN."
    } else if lower.contains("host key verification failed") {
        " Host-key verification failed. Connect once with ssh in a terminal and verify the host key."
    } else {
        ""
    };

    let code = status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".into());

    if detail.is_empty() {
        format!("{label} failed with exit status {code}.{hint}")
    } else {
        format!("{label} failed with exit status {code}: {detail}.{hint}")
    }
}

fn clean_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    const LIMIT: usize = 800;

    if collapsed.chars().count() <= LIMIT {
        return collapsed;
    }

    let shortened = collapsed.chars().take(LIMIT).collect::<String>();
    format!("{shortened}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_leaves_safe_arguments_readable() {
        assert_eq!(shell_quote("job-123_4").unwrap(), "job-123_4");
    }

    #[test]
    fn shell_quote_protects_tilde_spaces_and_single_quotes() {
        assert_eq!(shell_quote("~/ondemand/data").unwrap(), "'~/ondemand/data'");
        assert_eq!(shell_quote("hello world").unwrap(), "'hello world'");
        assert_eq!(shell_quote("it's").unwrap(), "'it'\"'\"'s'");
    }

    #[test]
    fn shell_quote_rejects_newlines() {
        assert!(shell_quote("unsafe\nargument").is_err());
    }
}
