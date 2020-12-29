use crate::config::ServerConfig;
use crate::daemon::{LogService, OutputState, Server};
use crate::ServerType;
use semver::Version;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, RwLock};

pub struct PaperServer {
    config: ServerConfig,
    input: Option<ChildStdin>,
}

impl Server for PaperServer {
    fn spawn(&mut self, log_service: &mut dyn LogService) -> (Child, Arc<RwLock<OutputState>>) {
        let mut child = Command::new("java".to_string())
            .arg("-jar")
            .arg(&self.config.jar)
            .arg("--nogui")
            .arg("--server-name")
            .arg(&self.config.name)
            .current_dir(&self.config.path)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.stdout.take();
        self.input = child.stdin.take();

        let status = log_service.manage_output(output.unwrap(), self.config.id.clone());

        (child, status)
    }

    fn send_command(&mut self, command: String) {
        if let Some(input) = &mut self.input {
            writeln!(input, "{}", command).unwrap();
        }
    }

    fn server_type(&self) -> ServerType {
        ServerType::Paper
    }

    fn version(&self) -> Version {
        self.config.version.clone()
    }

    fn path(&self) -> String {
        self.config.path.to_string_lossy().to_string()
    }
}

impl PaperServer {
    pub fn create(config: ServerConfig) -> Self {
        PaperServer {
            config,
            input: None,
        }
    }
}
