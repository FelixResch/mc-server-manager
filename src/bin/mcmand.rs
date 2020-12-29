use ipc_channel::ipc::IpcSender;
use mcman::ipc::{DaemonCmd, DaemonResponse, NewConnection, ServerEventType};
use std::sync::mpsc::{Receiver, channel, Sender};
use semver::Version;
use mcman::{fs::set_socket_name, ServerInfo, ServerStatus};
use mcman::config::DaemonConfig;
use std::path::Path;
use mcman::daemon::{Server, LogService, OutputState, DaemonEvent};
use std::collections::HashMap;
use std::process::Child;
use std::fs::remove_file;
use std::io::Read;
use std::sync::{RwLock, Arc, Mutex};
use std::thread::spawn;
use interprocess::local_socket::LocalSocketListener;
use std::ops::DerefMut;
use mcman::daemon::basic_log::BasicLogService;
use mcman::daemon::event::{EventManager, EventManagerCmd, EventHandler};

#[macro_use]
extern crate log;

fn main() {
    pretty_env_logger::init();
    let config = DaemonConfig::load(Path::new("mcman.toml"));
    debug!("config: {:?}", config);

    let (queue, daemon_queue) = channel();
    let (event_manager_ctrl, event_queue) = channel();
    let mut daemon = Daemon::from_config(config, daemon_queue, event_manager_ctrl.clone(), Box::new(BasicLogService::new(EventHandler::new(event_manager_ctrl.clone()))));

    daemon.autostart();

    let server_name = ".mcman.sock";
    let socket_path = Path::new(server_name);
    if socket_path.exists() {
        remove_file(socket_path).unwrap();
    }

    let listener = LocalSocketListener::bind(server_name).unwrap();
    set_socket_name(server_name);

    let senders = daemon.senders();
    let mut counter: u32 = 0;
    let mut receiver_buffer = Vec::with_capacity(64);

    let event_manager = EventManager::new(event_queue, queue.clone());
    event_manager.run();

    daemon.start_thread();

    while let Ok(mut rx) = listener.accept() {
        receiver_buffer.clear();
        match rx.read_to_end(&mut receiver_buffer) {
            Ok(bytes) => {
                debug!("read {} byte(s): {}", bytes, String::from_utf8_lossy(&receiver_buffer));
                match serde_json::from_slice::<NewConnection>(&receiver_buffer) {
                    Ok(new_con) => {
                        let NewConnection { min_version, client_version, socket_path, client_name } = new_con;
                        info!("client {} ({}) connected", client_name, client_version);
                        debug!("client requires minimum version {:?}", min_version);

                        let res_queue = IpcSender::connect(socket_path).unwrap();
                        let (sender, cmd_queue) = ipc_channel::ipc::channel::<DaemonCmd>().unwrap();

                        res_queue.send(DaemonResponse::SetSender { sender }).unwrap();
                        res_queue.send(DaemonResponse::Version { version: get_version() }).unwrap();
                        let event_queue = event_manager_ctrl.clone();

                        if let Some(min_version) = min_version {
                            if min_version > get_version() {
                                error!("can not satisfy version requirement {}", min_version);
                                continue;
                            }
                        }

                        let mut write = senders.lock().unwrap();

                        write.insert(counter, res_queue);
                        let queue_clone = queue.clone();
                        let id = counter;

                        spawn(move || {
                            while let Ok(cmd) = cmd_queue.recv() {
                                queue_clone.send(DaemonEvent::IncomingCmd {
                                    id,
                                    cmd,
                                }).unwrap();
                            }
                            event_queue.send(EventManagerCmd::RemoveAllSubscriptions {client_id: id}).unwrap();
                            debug!("ending client thread")
                        });
                        counter += 1;
                    }
                    Err(e) => warn!("could not deserialize connection descriptor: {}", e)
                }
            }
            Err(e) => warn!("error when initiating connection: {}", e)
        }
    }

}

struct Daemon {
    config: DaemonConfig,
    servers: HashMap<String, DaemonServer>,
    senders: Arc<Mutex<HashMap<u32, IpcSender<DaemonResponse>>>>,
    queue: Receiver<DaemonEvent>,
    log_service: Box<dyn LogService + Send>,
    event_manager_ctrl: Sender<EventManagerCmd>,
}

impl Daemon {

    pub fn from_config(daemon_config: DaemonConfig, queue: Receiver<DaemonEvent>, event_manager_ctrl: Sender<EventManagerCmd>, log_service: Box<dyn LogService + Send>) -> Self {
        let servers = daemon_config.create_servers();
        let mut daemon_servers = HashMap::with_capacity(servers.len());
        for (id, server) in servers.into_iter() {
            daemon_servers.insert(id.clone(), DaemonServer {
                server,
                process: None,
                status: None,
                server_id: id,
            });
        }

        Daemon {
            config: daemon_config,
            servers: daemon_servers,
            senders: Arc::new(Mutex::new(HashMap::new())),
            queue,
            log_service,
            event_manager_ctrl,
        }
    }

    pub fn autostart(&mut self) {
        if !self.config.autostart.is_empty() {
            info!("performing autostart");
        }
        for server_id in &self.config.autostart {
            if let Some(server) = self.servers.get_mut(server_id.as_str()) {
                server.start(self.log_service.deref_mut());
            }
        }
    }

    #[allow(dead_code)]
    pub fn wait_all(&mut self) {
        for (id, server) in &mut self.servers {
            if let Some(child) = &mut server.process {
                debug!("{}: {:?}", id, child.wait())
            }
        }
    }

    pub fn handle_cmd(&mut self, cmd: DaemonCmd, client_id: u32) -> DaemonResponse {
        match cmd {
            DaemonCmd::List => {
                let list = self.servers.iter_mut()
                    .map(|(id, server)| {
                        ServerInfo {
                            path: server.server.path(),
                            name: id.clone(),
                            server_status: server.status(),
                            server_version: server.server.version(),
                            server_type: server.server.server_type(),
                        }
                    })
                    .collect();
                DaemonResponse::List {
                    servers: list,
                }
            },
            DaemonCmd::GetVersion => DaemonResponse::Version {
                version: get_version()
            },
            DaemonCmd::Start { server_id, wait } => {
                let server = self.servers.get_mut(server_id.as_str());
                if let Some(server) = server {
                    if let ServerStatus::Down = server.status() {
                        server.start(self.log_service.deref_mut());
                    }
                    if wait {
                        self.subscribe_event(ServerEventType::ServerStarting, Some(vec![server_id.clone()]), client_id);
                        self.subscribe_event(ServerEventType::ServerStarted, Some(vec![server_id.clone()]), client_id);
                        DaemonResponse::Ok
                    } else {
                        DaemonResponse::ServerStarted { server_id }
                    }
                } else {
                    DaemonResponse::ServerNotFound { server_id }
                }
            }
            DaemonCmd::Stop { server_id, wait } => {
                let server = self.servers.get_mut(server_id.as_str());
                if let Some(server) = server {
                    if let ServerStatus::Running = server.status() {
                        server.stop();
                    }
                    if wait {
                        self.subscribe_event(ServerEventType::ServerStopping, Some(vec![server_id.clone()]), client_id);
                        self.subscribe_event(ServerEventType::ServerStopped, Some(vec![server_id.clone()]), client_id);
                        DaemonResponse::Ok
                    } else {
                        DaemonResponse::ServerStopped { server_id }
                    }
                } else {
                    DaemonResponse::ServerNotFound { server_id }
                }
            }
            DaemonCmd::SubscribeEvent { event_type, server_ids: server_names } => {
                self.subscribe_event(event_type, server_names, client_id);
                DaemonResponse::Ok
            }
        }
    }

    pub fn subscribe_event(&mut self, event_type: ServerEventType, server_ids: Option<Vec<String>>, client_id: u32) {
        if let Some(server_ids) = server_ids {
            for server_id in server_ids {
                self.event_manager_ctrl.send(EventManagerCmd::AddSubscription {
                    server_id,
                    event_type,
                    client_id
                }).unwrap();
            }
        }
    }

    pub fn senders(&self) -> Arc<Mutex<HashMap<u32, IpcSender<DaemonResponse>>>> {
        self.senders.clone()
    }

    pub fn start_thread(mut self) {
        spawn(move || {
            while let Ok(cmd) = self.queue.recv() {
                match cmd {
                    DaemonEvent::IncomingCmd { id, cmd} => {
                        let response = self.handle_cmd(cmd, id);

                        let mut senders = self.senders.lock().unwrap();
                        let sender = senders.get_mut(&id).unwrap();
                        match sender.send(response) {
                            Ok(_) => {}
                            Err(_) => {
                                self.event_manager_ctrl.send(EventManagerCmd::RemoveAllSubscriptions { client_id: id }).unwrap();
                            }
                        }
                    },
                    DaemonEvent::SendEvent { client_id, event } => {
                        let mut senders = self.senders.lock().unwrap();
                        let sender = senders.get_mut(&client_id).unwrap();
                        match sender.send(DaemonResponse::ServerEvent {event}) {
                            Ok(_) => {}
                            Err(_) => {
                                self.event_manager_ctrl.send(EventManagerCmd::RemoveAllSubscriptions { client_id }).unwrap();
                            }
                        }
                    }
                }
            }
        });
    }
}

struct DaemonServer {
    process: Option<Child>,
    server: Box<dyn Server + Send + 'static>,
    status: Option<Arc<RwLock<OutputState>>>,
    server_id: String,
}

impl DaemonServer {

    pub fn start(&mut self, log_service: &mut (dyn LogService + Send)) {
        debug!("starting unit {}", self.server_id);
        let (child, status) = self.server.spawn(log_service);
        self.process = Some(child);
        self.status = Some(status);
    }

    pub fn status(&mut self) -> ServerStatus {
        if let Some(child) = &mut self.process {
            match child.try_wait() {
                Ok(Some(status)) => if status.success() {
                    ServerStatus::Down
                } else {
                    ServerStatus::Errored(status.code())
                },
                _ => {
                    if let Some(status) = &self.status {
                        let guard = status.read().unwrap();
                        match *guard {
                            OutputState::Unknown => ServerStatus::Unknown,
                            OutputState::Starting => ServerStatus::Starting,
                            OutputState::Started => ServerStatus::Running,
                            OutputState::Errored => ServerStatus::Errored(None),
                            OutputState::Stopped => ServerStatus::Down,
                            OutputState::Stopping => ServerStatus::Stopping,
                        }
                    } else {
                        ServerStatus::Unknown
                    }
                }
            }
        } else {
            ServerStatus::Down
        }
    }

    pub fn send_command(&mut self, command: String) {
        self.server.send_command(command);
    }

    pub fn stop(&mut self) {
        self.send_command("stop".to_string())
    }
}

fn get_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
}