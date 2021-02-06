//! Implementations for the PaperMC server software.

use crate::config::{ServerConfig, ServerUnitConfig, UnitConfig};
use crate::daemon::{LogService, OutputState, Server};
use crate::{ServerType, Unit};
use semver::Version;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, RwLock};

/// A PaperMC server
pub struct PaperServer {
    /// The config of this server
    config: ServerConfig,
    unit_config: UnitConfig,
    /// The input of the current server process
    input: Option<ChildStdin>,
    unit_file: PathBuf,
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

        let status = log_service.manage_output(output.unwrap(), self.unit_config.id.clone());

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

    fn server_config(&self) -> ServerConfig {
        self.config.clone()
    }
}

impl PaperServer {
    /// Creates a new server from the given server config
    pub fn create(unit_config: UnitConfig, config: ServerConfig, unit_file: PathBuf) -> Self {
        PaperServer {
            config,
            unit_config,
            input: None,
            unit_file,
        }
    }
}

impl Unit for PaperServer {
    fn unit_file_path(&self) -> PathBuf {
        self.unit_file.clone()
    }

    fn unit_config(&self) -> UnitConfig {
        self.unit_config.clone()
    }
}
