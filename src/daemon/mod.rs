//! Structs and traits used by the daemon.

pub mod basic_log;
pub mod event;
pub mod paper;

use crate::config::{ServerConfig, ServerUnitConfig};
use crate::daemon::paper::PaperServer;
use crate::ipc::{DaemonCmd, ServerEvent, DaemonIpcEvent};
use crate::{ServerType, Unit};
use log::warn;
use semver::Version;
use std::path::PathBuf;
use std::process::{Child, ChildStdout};
use std::sync::{Arc, RwLock};

/// A server manages by the daemon.
///
/// Implementations are:
/// - [`paper::PaperServer`]
pub trait Server
where
    Self: Unit,
{
    /// Start a process for this server.
    /// `log_service` contains a log service which parses the output to update the server status.
    ///
    /// The spawned process must be returned, together with a [`RwLock`] for the [`OutputState`] of the server.
    //TODO the lock should be passed somewhere else, the way it is passed now feels wrong
    fn spawn(&mut self, log_service: &mut dyn LogService) -> (Child, Arc<RwLock<OutputState>>);

    /// Send a command to a running instance of the server.
    fn send_command(&mut self, command: String);

    /// Returns the type of the server.
    ///
    /// > `&self` is required to be object safe.
    fn server_type(&self) -> ServerType;

    /// Returns the version of the server software
    fn version(&self) -> Version;

    /// Returns the path to the server directory
    fn path(&self) -> String;

    fn server_config(&self) -> ServerConfig;
}

/// State of a Minecraft server process based on the log output.
///
/// Once a server has reached the state [`OutputState::Errored`] it cannot change to another state by log output.
pub enum OutputState {
    /// The state of the server cannot be determined from the log output
    Unknown,
    /// The server is starting
    Starting,
    /// The server has started
    Started,
    /// The server has encountered an error (e..g Exception, panic, ...)
    Errored,
    /// The server is shutting down.
    Stopping,
    /// The server has shut down.
    Stopped,
}

/// Service to parse the log output of a Minecraft server and updates the server state while doing so.
pub trait LogService {
    /// Manage the output of a process.
    /// This call must return the lock and update it whenever the log output of the server suggests the the state of the server has changed.
    fn manage_output(&mut self, out: ChildStdout, server_name: String) -> Arc<RwLock<OutputState>>;
}

/// Event for the main daemon thread.
pub enum DaemonEvent {
    /// An incoming command from a client
    IncomingCmd {
        /// The id of the client that sent the command.
        id: u32,
        /// The received command.
        cmd: DaemonCmd,
    },
    /// Raise an event for a given client
    SendEvent {
        /// The id of the client to which the event should be sent to
        client_id: u32,
        /// The event that should be sent to the client
        event: ServerEvent,
    },
    AddServerUnit {
        server_unit_config: ServerUnitConfig,
        unit_file: PathBuf,
    },
    StopDaemon,
    SendDaemonEvent(DaemonIpcEvent)
}

//TODO proper error type
pub fn create_server(
    server_unit_config: ServerUnitConfig,
    unit_file: PathBuf,
) -> Result<Box<dyn Server + Send>, ()> {
    match server_unit_config.server.type_name.as_str() {
        "paper" => {
            let ServerUnitConfig { unit, server } = server_unit_config;
            let server = PaperServer::create(unit, server, unit_file);
            Ok(Box::new(server))
        }
        _ => {
            warn!(
                "unknown server type {}",
                server_unit_config.server.type_name
            );
            Err(())
        }
    }
}
