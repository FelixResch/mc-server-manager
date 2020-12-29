use ipc_channel::ipc::IpcSender;
use semver::Version;
use crate::ServerInfo;

#[derive(Serialize, Debug, Deserialize)]
pub enum DaemonCmd {
    List,
    GetVersion,
    Start {
        server_id: String,
        /// If true the client is automatically subscribed to the events `ServerStarting` and `ServerStarting`
        /// of the server specified in `server_name`
        wait: bool,
    },
    Stop {
        server_id: String,
        /// If true the client is automatically subscribed to the events `ServerStopping` and `ServerStopped`
        /// of the server specified in `server_name`
        wait: bool,
    },
    SubscribeEvent {
        event_type: ServerEventType,
        server_ids: Option<Vec<String>>,
    }
}

#[derive(Serialize, Debug, Deserialize)]
pub enum DaemonResponse {
    List {
        servers: Vec<ServerInfo>,
    },
    UnknownCommand,
    Version {
        version: Version,
    },
    SetSender {
        sender: IpcSender<DaemonCmd>,
    },
    ServerNotFound {
        server_id: String,
    },
    ServerStarted {
        server_id: String,
    },
    ServerStopped {
        server_id: String,
    },
    ServerEvent {
        event: ServerEvent,
    },
    Ok,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewConnection {
    pub min_version: Option<Version>,
    pub client_version: Version,
    pub socket_path: String,
    pub client_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerEvent {
    ServerStarting {
        server_id: String,
    },
    ServerStarted {
        server_id: String,
    },
    ServerStopping {
        server_id: String,
    },
    ServerStopped {
        server_id: String,
    },
}

impl ServerEvent {

    pub fn get_event_type(&self) -> ServerEventType {
        match self {
            ServerEvent::ServerStarting { .. } => ServerEventType::ServerStarting,
            ServerEvent::ServerStarted { .. } => ServerEventType::ServerStarted,
            ServerEvent::ServerStopping { .. } => ServerEventType::ServerStopping,
            ServerEvent::ServerStopped { .. } => ServerEventType::ServerStopped,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ServerEventType {
    ServerStarting,
    ServerStarted,
    ServerStopping,
    ServerStopped,
}