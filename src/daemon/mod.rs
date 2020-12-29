pub mod basic_log;
pub mod event;
pub mod paper;

use crate::ipc::{DaemonCmd, ServerEvent};
use crate::ServerType;
use semver::Version;
use std::process::{Child, ChildStdout};
use std::sync::{Arc, RwLock};

pub trait Server {
    fn spawn(&mut self, log_service: &mut dyn LogService) -> (Child, Arc<RwLock<OutputState>>);

    fn send_command(&mut self, command: String);

    fn server_type(&self) -> ServerType;

    fn version(&self) -> Version;

    fn path(&self) -> String;
}

pub enum OutputState {
    Unknown,
    Starting,
    Started,
    Errored,
    Stopping,
    Stopped,
}

pub trait LogService {
    fn manage_output(&mut self, out: ChildStdout, server_name: String) -> Arc<RwLock<OutputState>>;
}

pub enum DaemonEvent {
    IncomingCmd { id: u32, cmd: DaemonCmd },
    SendEvent { client_id: u32, event: ServerEvent },
}
