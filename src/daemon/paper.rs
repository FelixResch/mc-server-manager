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
            .arg(format!("-Xms{}G", self.server_config().memory))
            .arg(format!("-Xmx{}G", self.server_config().memory))
            .arg("-XX:+UseG1GC")
            .arg("-XX:+ParallelRefProcEnabled")
            .arg("-XX:MaxGCPauseMillis=200")
            .arg("-XX:+UnlockExperimentalVMOptions")
            .arg("-XX:+DisableExplicitGC")
            .arg("-XX:+AlwaysPreTouch")
            .arg("-XX:G1NewSizePercent=30")
            .arg("-XX:G1MaxNewSizePercent=40")
            .arg("-XX:G1HeapRegionSize=8M")
            .arg("-XX:G1ReservePercent=20")
            .arg("-XX:G1HeapWastePercent=5")
            .arg("-XX:G1MixedGCCountTarget=4")
            .arg("-XX:InitiatingHeapOccupancyPercent=15")
            .arg("-XX:G1MixedGCLiveThresholdPercent=90")
            .arg("-XX:G1RSetUpdatingPauseTimePercent=5")
            .arg("-XX:SurvivorRatio=32")
            .arg("-XX:+PerfDisableSharedMem")
            .arg("-XX:MaxTenuringThreshold=1")
            .arg("-Dusing.aikars.flags=https://mcflags.emc.gs")
            .arg("-Daikars.new.flags=true")
            .arg("-jar")
            .arg(&self.config.jar)
            .arg("--nogui")
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
