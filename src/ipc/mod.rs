use ipc_channel::ipc::IpcSender;
use semver::Version;
use crate::ServerInfo;

#[derive(Serialize, Debug, Deserialize)]
pub enum DaemonCmd {
    Status {
        name: Option<String>
    },
    List,
    GetVersion,
    SetSender(IpcSender<DaemonResponse>)
}

#[derive(Serialize, Debug, Deserialize)]
pub enum DaemonResponse {
    List {
        servers: Vec<ServerInfo>,
    },
    UnknownCommand,
    Version {
        version: Version,
    }
}