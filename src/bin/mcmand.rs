use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcReceiverSet};
use mcman::ipc::{DaemonCmd, DaemonResponse};
use std::sync::mpsc::Receiver;
use semver::Version;
use mcman::{set_socket_name, ServerInfo, ServerType, ServerStatus};

fn main() {
    let (server, server_name) = IpcOneShotServer::new().unwrap();
    set_socket_name(&server_name);
    let (rx, cmd): (IpcReceiver<DaemonCmd>, _) = server.accept().unwrap();

    let mut daemon = Daemon {};

    if let DaemonCmd::SetSender(tx) = cmd {
        tx.send(DaemonResponse::Version {version: get_version()});
        while let Ok(cmd) = rx.recv() {
            let response = daemon.handle_cmd(cmd);
            tx.send(response).unwrap();
        }
    } else {
        eprintln!("expected SetSender daemon command, got: {:?}", cmd)
    }
}

struct Daemon {
}

impl Daemon {

    fn handle_cmd(&mut self, cmd: DaemonCmd) -> DaemonResponse {
        match cmd {
            DaemonCmd::Status { .. } => DaemonResponse::UnknownCommand,
            DaemonCmd::List => DaemonResponse::List {
                servers: vec![
                    ServerInfo {
                        name: "paper1".to_string(),
                        path: "/opt/mc/paper1".to_string(),
                        server_type: ServerType::Paper,
                        server_version: Version::new(16, 4, 1),
                        server_status: ServerStatus::Updating,
                    }
                ]
            },
            DaemonCmd::GetVersion => DaemonResponse::Version {
                version: get_version()
            },
            DaemonCmd::SetSender(_) => DaemonResponse::UnknownCommand,
        }
    }
}

fn get_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
}