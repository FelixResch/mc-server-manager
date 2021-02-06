//! Primitives for daemon client inter-process communication.

pub mod install;
pub mod update;

use crate::ipc::install::InstallError;
use crate::{ServerInfo, ServerType};
use ipc_channel::ipc::IpcSender;
use semver::Version;

/// Commands sent from the client to the daemon.
/// The expected responses are (/will be) documented in a separate document.
#[derive(Serialize, Debug, Deserialize)]
pub enum DaemonCmd {
    /// List all currently managed units
    List,
    /// Get the version of the daemon
    GetVersion,
    /// Start a server
    Start {
        /// The server id of the server to start
        server_id: String,
        /// If true the client is automatically subscribed to the events `ServerStarting` and `ServerStarting`
        /// of the server specified in `server_name`
        wait: bool,
    },
    /// Stop a server
    Stop {
        /// The server id of the server to stop
        server_id: String,
        /// If true the client is automatically subscribed to the events `ServerStopping` and `ServerStopped`
        /// of the server specified in `server_name`
        wait: bool,
    },
    /// Send events of the the given type for the given servers to the client.
    /// If [`server_ids`] is `None` the subscription is made for all servers.
    SubscribeEvent {
        /// The type of event, the client wants to subscribe to
        event_type: ServerEventType,
        /// The ids of the servers the client want to listen on.
        /// If this value is `None` the client wants to receive events from all servers.
        server_ids: Option<Vec<String>>,
    },
    InstallServer {
        unit_id: String,
        install_path: String,
        unit_file_path: Option<String>,
        server_version: Option<Version>,
        server_type: ServerType,
        accept_eula: bool,
        server_name: Option<String>,
    },
    UpdateServer {
        unit_id: String,
        server_version: Option<Version>,
    },
}

/// Responses sent from the daemon to a client
#[derive(Serialize, Debug, Deserialize)]
pub enum DaemonResponse {
    /// A list of all currently managed units
    List {
        /// The units that are currently managed by the daemon
        servers: Vec<ServerInfo>,
    },
    /// The command that was sent was not known or is not implemented.
    ///
    /// This error should not occur, when the daemon is used with the provided client, because
    /// `bincode` encoding is used.
    UnknownCommand,
    /// The version of the daemon is sent.
    ///
    /// The daemon sends this response at beginning of every connection after checking that the version requirements are met.
    Version {
        /// The version of the daemon
        version: Version,
    },
    /// Set the sender of the client. This currently a requirement of the IPC crate used.
    SetSender {
        /// The sender the client should use
        sender: IpcSender<DaemonCmd>,
    },
    /// The given server id could not be found
    ServerNotFound {
        /// The server id that could not be found
        server_id: String,
    },
    /// The server identified by [`server_id`] has been started.
    ///
    /// Note that this does not mean, that the server is already accepting connections.
    /// When the daemon sends this response it only means, that the process has been started.
    ServerStarted {
        /// The server id of the server that has been started
        server_id: String,
    },
    /// The server identified by [`server_id`] has been started.
    ///
    /// Note that this does not mean, that the server has completely shut off.
    /// This only means, that the `stop` command has been sent to the server.
    ServerStopped {
        /// The server that has been stopped.
        server_id: String,
    },
    /// An event has occurred which the client has subscribed to
    ServerEvent {
        /// The event that has happened
        event: ServerEvent,
    },
    /// Acknowledges any prior command, which does not have a direct response.
    ///
    /// Note: This response does not specify to which request it belongs, this should be changed.
    Ok,
}

/// Information for a new connection used when establishing a new connection to the daemon.
#[derive(Serialize, Deserialize, Debug)]
pub struct NewConnection {
    /// The minimum required version of the daemon.
    ///
    /// The daemon will reject session when the version requirement is not met.
    pub min_version: Option<Version>,
    /// The version of the client.
    pub client_version: Version,
    /// The path of a [`ipc_channel::ipc::IpcOneShotServer<DaemonResponse>`] created by the client
    pub socket_path: String,
    /// The name of the client software
    pub client_name: String,
}

/// Events occurring on the server
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerEvent {
    /// A server has entered the starting stage
    ServerStarting {
        /// The id of the server that has entered the starting stage
        server_id: String,
    },
    /// A server has started (and is accepting connections)
    ServerStarted {
        /// The id of the server that has signalled that it is ready to receive connections
        server_id: String,
    },
    /// A server has entered the stopping stage
    ServerStopping {
        /// The id of the server that has begun to shut down
        server_id: String,
    },
    /// A server has stopped
    ServerStopped {
        /// The id of the server that has stopped
        server_id: String,
    },
    ActionProgress {
        server_id: String,
        action: String,
        progress: Option<usize>,
        maximum: Option<usize>,
        action_number: usize,
    },
    InstallationComplete {
        server_id: String,
    },
    InstallationFailed {
        server_id: String,
        error: String,
    },
    UpdateComplete {
        server_id: String,
    },
    UpdateFailed {
        server_id: String,
        error: String,
    },
}

impl ServerEvent {
    /// Returns the [`ServerEventType`] associated with a specific type of event
    pub fn get_event_type(&self) -> ServerEventType {
        match self {
            ServerEvent::ServerStarting { .. } => ServerEventType::ServerStarting,
            ServerEvent::ServerStarted { .. } => ServerEventType::ServerStarted,
            ServerEvent::ServerStopping { .. } => ServerEventType::ServerStopping,
            ServerEvent::ServerStopped { .. } => ServerEventType::ServerStopped,
            ServerEvent::ActionProgress { .. } => ServerEventType::ActionProgress,
            ServerEvent::InstallationComplete { .. } => ServerEventType::InstallationComplete,
            ServerEvent::InstallationFailed { .. } => ServerEventType::InstallationFailed,
            ServerEvent::UpdateComplete { .. } => ServerEventType::UpdateComplete,
            ServerEvent::UpdateFailed { .. } => ServerEventType::UpdateFailed,
        }
    }
}

/// Types of server events.
///
/// There exists a 1:1 mapping between [`ServerEvent`]s and [`ServerEventType`]s, defined by
/// [`ServerEvent::get_event_type(&self)`].
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ServerEventType {
    /// A server has entered the starting stage
    ServerStarting,
    /// A server has started (and is accepting connections)
    ServerStarted,
    /// A server has entered the stopping stage
    ServerStopping,
    /// A server has stopped
    ServerStopped,
    ActionProgress,
    InstallationComplete,
    InstallationFailed,
    UpdateComplete,
    UpdateFailed,
}
