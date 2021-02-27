use interprocess::local_socket::LocalSocketListener;
use ipc_channel::ipc::IpcSender;
use mcman::config::{DaemonConfig, ServerConfig, ServerUnitConfig, UnitConfig};
use mcman::daemon::basic_log::BasicLogService;
use mcman::daemon::event::{EventHandler, EventManager, EventManagerCmd};
use mcman::daemon::{create_server, DaemonEvent, LogService, OutputState, Server};
use mcman::ipc::install::{InstallError, PaperServerInstaller, ServerInstaller};
use mcman::ipc::update::UpdateError::UnsupportedServerType;
use mcman::ipc::update::{PaperServerUpdater, ServerUpdater, UpdateError};
use mcman::ipc::{
    DaemonCmd, DaemonIpcEvent, DaemonResponse, NewConnection, ServerEvent, ServerEventType,
};
use mcman::{ServerInfo, ServerStatus, ServerType};
#[cfg(feature = "systemd")]
use sd_notify::NotifyState;
use semver::Version;
use std::collections::HashMap;
use std::fs;
use std::fs::remove_file;
use std::io::Read;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};
use std::process::{exit, Child};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{sleep, spawn};
use std::time::Duration;

#[macro_use]
extern crate log;

fn main() {
    pretty_env_logger::init();
    let config = DaemonConfig::load(Path::new("mcman.toml"));
    debug!("config: {:?}", config);

    let server_name = config.socket_file.clone();

    let (queue, daemon_queue) = channel();
    let (event_manager_ctrl, event_queue) = channel();
    let mut daemon = Daemon::from_config(
        config,
        daemon_queue,
        queue.clone(),
        event_manager_ctrl.clone(),
        Box::new(BasicLogService::new(EventHandler::new(
            event_manager_ctrl.clone(),
        ))),
    );

    daemon.autostart();

    let socket_path = Path::new(server_name.as_str());
    if socket_path.exists() {
        remove_file(socket_path).unwrap();
    }

    let listener = LocalSocketListener::bind(server_name).unwrap();

    let senders = daemon.senders();
    let mut counter: u32 = 0;
    let mut receiver_buffer = Vec::with_capacity(64);

    let event_manager = EventManager::new(event_queue, queue.clone());
    event_manager.run();

    daemon.start_thread();

    #[cfg(feature = "systemd")]
    if let Ok(true) = sd_notify::booted() {
        if let Ok(ctrl) = std::env::var("MCMAND_CTRL") {
            if ctrl == "systemd" {
                debug!("systemd detected, notifying systemd");
                let _ = sd_notify::notify(false, &[NotifyState::Ready]);
            }
        }
    }

    while let Ok(mut rx) = listener.accept() {
        receiver_buffer.clear();
        match rx.read_to_end(&mut receiver_buffer) {
            Ok(bytes) => {
                debug!(
                    "read {} byte(s): {}",
                    bytes,
                    String::from_utf8_lossy(&receiver_buffer)
                );
                match serde_json::from_slice::<NewConnection>(&receiver_buffer) {
                    Ok(new_con) => {
                        let NewConnection {
                            min_version,
                            client_version,
                            socket_path,
                            client_name,
                        } = new_con;
                        info!("client {} ({}) connected", client_name, client_version);
                        debug!("client requires minimum version {:?}", min_version);

                        let connect_result = IpcSender::connect(socket_path);
                        if let Ok(res_queue) = connect_result {
                            let (sender, cmd_queue) =
                                ipc_channel::ipc::channel::<DaemonCmd>().unwrap();

                            res_queue
                                .send(DaemonResponse::SetSender { sender })
                                .unwrap();
                            res_queue
                                .send(DaemonResponse::Version {
                                    version: get_version(),
                                })
                                .unwrap();
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
                                    queue_clone
                                        .send(DaemonEvent::IncomingCmd { id, cmd })
                                        .unwrap();
                                }
                                event_queue
                                    .send(EventManagerCmd::RemoveAllSubscriptions { client_id: id })
                                    .unwrap();
                                debug!("ending client thread")
                            });
                            counter += 1;
                        } else {
                            warn!("error on incoming connection {:?}", connect_result)
                        }
                    }
                    Err(e) => warn!("could not deserialize connection descriptor: {}", e),
                }
            }
            Err(e) => warn!("error when initiating connection: {}", e),
        }
    }
}

struct Daemon {
    config: DaemonConfig,
    servers: HashMap<String, DaemonServer>,
    senders: Arc<Mutex<HashMap<u32, IpcSender<DaemonResponse>>>>,
    queue: Receiver<DaemonEvent>,
    queue_sender: Sender<DaemonEvent>,
    log_service: Box<dyn LogService + Send>,
    event_manager_ctrl: Sender<EventManagerCmd>,
}

impl Daemon {
    pub fn from_config(
        daemon_config: DaemonConfig,
        queue: Receiver<DaemonEvent>,
        queue_sender: Sender<DaemonEvent>,
        event_manager_ctrl: Sender<EventManagerCmd>,
        log_service: Box<dyn LogService + Send>,
    ) -> Self {
        let servers = daemon_config.create_servers();
        let mut daemon_servers = HashMap::with_capacity(servers.len());
        for (id, server) in servers.into_iter() {
            daemon_servers.insert(
                id.clone(),
                DaemonServer {
                    server,
                    process: None,
                    status: None,
                    server_id: id,
                },
            );
        }

        Daemon {
            config: daemon_config,
            servers: daemon_servers,
            senders: Arc::new(Mutex::new(HashMap::new())),
            queue,
            queue_sender,
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
                let list = self
                    .servers
                    .iter_mut()
                    .map(|(id, server)| ServerInfo {
                        path: server.server.path(),
                        name: id.clone(),
                        server_status: server.status(),
                        server_version: server.server.version(),
                        server_type: server.server.server_type(),
                    })
                    .collect();
                DaemonResponse::List { servers: list }
            }
            DaemonCmd::GetVersion => DaemonResponse::Version {
                version: get_version(),
            },
            DaemonCmd::Start { server_id, wait } => {
                let server = self.servers.get_mut(server_id.as_str());
                if let Some(server) = server {
                    if let ServerStatus::Down = server.status() {
                        server.start(self.log_service.deref_mut());
                    }
                    if wait {
                        self.subscribe_event(
                            ServerEventType::ServerStarting,
                            Some(vec![server_id.clone()]),
                            client_id,
                        );
                        self.subscribe_event(
                            ServerEventType::ServerStarted,
                            Some(vec![server_id.clone()]),
                            client_id,
                        );
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
                        self.subscribe_event(
                            ServerEventType::ServerStopping,
                            Some(vec![server_id.clone()]),
                            client_id,
                        );
                        self.subscribe_event(
                            ServerEventType::ServerStopped,
                            Some(vec![server_id.clone()]),
                            client_id,
                        );
                        DaemonResponse::Ok
                    } else {
                        DaemonResponse::ServerStopped { server_id }
                    }
                } else {
                    DaemonResponse::ServerNotFound { server_id }
                }
            }
            DaemonCmd::SubscribeEvent {
                event_type,
                server_ids: server_names,
            } => {
                self.subscribe_event(event_type, server_names, client_id);
                DaemonResponse::Ok
            }
            DaemonCmd::InstallServer {
                unit_id,
                install_path,
                unit_file_path,
                server_version,
                server_type,
                accept_eula,
                server_name,
            } => {
                self.subscribe_event(
                    ServerEventType::InstallationComplete,
                    Some(vec![unit_id.clone()]),
                    client_id,
                );
                self.subscribe_event(
                    ServerEventType::InstallationFailed,
                    Some(vec![unit_id.clone()]),
                    client_id,
                );
                self.subscribe_event(
                    ServerEventType::ActionProgress,
                    Some(vec![unit_id.clone()]),
                    client_id,
                );
                self.install_server(
                    EventHandler::new(self.event_manager_ctrl.clone()),
                    unit_id.clone(),
                    install_path,
                    unit_file_path,
                    server_version,
                    server_type,
                    accept_eula,
                    server_name,
                    self.queue_sender.clone(),
                );
                DaemonResponse::Ok
            }
            DaemonCmd::UpdateServer {
                unit_id,
                server_version,
            } => {
                let unit = self.servers.get(&unit_id);
                if let Some(unit) = unit {
                    let unit_file_path = unit.server.unit_file_path();
                    let server_type = unit.server.server_type();
                    let unit_config = unit.server.unit_config();
                    let server_config = unit.server.server_config();

                    self.subscribe_event(
                        ServerEventType::UpdateComplete,
                        Some(vec![unit_id.clone()]),
                        client_id,
                    );
                    self.subscribe_event(
                        ServerEventType::UpdateFailed,
                        Some(vec![unit_id.clone()]),
                        client_id,
                    );
                    self.subscribe_event(
                        ServerEventType::ActionProgress,
                        Some(vec![unit_id.clone()]),
                        client_id,
                    );

                    self.update_server(
                        EventHandler::new(self.event_manager_ctrl.clone()),
                        unit_id,
                        server_version,
                        server_type,
                        unit_config,
                        server_config,
                        unit_file_path,
                        self.queue_sender.clone(),
                    );

                    DaemonResponse::Ok
                } else {
                    DaemonResponse::ServerNotFound { server_id: unit_id }
                }
            }
            DaemonCmd::StopDaemon => {
                self.queue_sender.send(DaemonEvent::StopDaemon).unwrap();
                DaemonResponse::Ok
            }
            DaemonCmd::SendMessage { unit_id, message } => {
                let unit = self.servers.get_mut(&unit_id);
                match unit {
                    Some(server) => {
                        server.say(message);
                        DaemonResponse::Ok
                    }
                    None => DaemonResponse::ServerNotFound { server_id: unit_id },
                }
            }
        }
    }

    pub fn subscribe_event(
        &mut self,
        event_type: ServerEventType,
        server_ids: Option<Vec<String>>,
        client_id: u32,
    ) {
        if let Some(server_ids) = server_ids {
            for server_id in server_ids {
                self.event_manager_ctrl
                    .send(EventManagerCmd::AddSubscription {
                        server_id,
                        event_type,
                        client_id,
                    })
                    .unwrap();
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
                    DaemonEvent::IncomingCmd { id, cmd } => {
                        let response = self.handle_cmd(cmd, id);

                        let mut senders = self.senders.lock().unwrap();
                        let sender = senders.get_mut(&id).unwrap();
                        match sender.send(response) {
                            Ok(_) => {}
                            Err(_) => {
                                self.event_manager_ctrl
                                    .send(EventManagerCmd::RemoveAllSubscriptions { client_id: id })
                                    .unwrap();
                            }
                        }
                    }
                    DaemonEvent::SendEvent { client_id, event } => {
                        let mut senders = self.senders.lock().unwrap();
                        let sender = senders.get_mut(&client_id).unwrap();
                        match sender.send(DaemonResponse::ServerEvent { event }) {
                            Ok(_) => {}
                            Err(_) => {
                                self.event_manager_ctrl
                                    .send(EventManagerCmd::RemoveAllSubscriptions { client_id })
                                    .unwrap();
                            }
                        }
                    }
                    DaemonEvent::AddServerUnit {
                        server_unit_config,
                        unit_file,
                    } => {
                        let unit_id = server_unit_config.unit.id.clone();
                        match create_server(server_unit_config, unit_file) {
                            Ok(server) => {
                                self.servers.insert(
                                    unit_id.clone(),
                                    DaemonServer {
                                        process: None,
                                        server,
                                        status: None,
                                        server_id: unit_id,
                                    },
                                );
                            }
                            _ => (),
                        }
                    }
                    DaemonEvent::StopDaemon => {
                        #[cfg(feature = "systemd")]
                        if let Ok(true) = sd_notify::booted() {
                            if let Ok(ctrl) = std::env::var("MCMAND_CTRL") {
                                if ctrl == "systemd" {
                                    debug!("systemd detected, notifying systemd");
                                    let _ = sd_notify::notify(false, &[NotifyState::Stopping]);
                                }
                            }
                        }

                        self.servers
                            .iter_mut()
                            .map(|(unit_id, server)| {
                                debug!("Stopping unit {}", unit_id);
                                match server.status() {
                                    ServerStatus::Starting => {
                                        if server.has_started() {
                                            (unit_id, server.stop())
                                        } else {
                                            (unit_id, None)
                                        }
                                    }
                                    ServerStatus::Running => (unit_id, server.stop()),
                                    ServerStatus::Updating => {
                                        panic!("currently no strategy implemented!")
                                    }
                                    ServerStatus::Lockdown => {
                                        panic!("currently no strategy implemented!")
                                    }
                                    _ => {
                                        debug!("Nothing to do for unit {}", unit_id);
                                        (unit_id, None)
                                    }
                                }
                            })
                            .for_each(|(unit_id, child)| {
                                if let Some(mut child) = child {
                                    let exit_status = child.wait();
                                    debug!(
                                        "unit {} stopped with exit status {:?}",
                                        unit_id, exit_status
                                    );
                                }
                            });
                        self.queue_sender
                            .send(DaemonEvent::SendDaemonEvent(DaemonIpcEvent::Stopped))
                            .expect("send to own event queue");
                    }
                    DaemonEvent::SendDaemonEvent(DaemonIpcEvent::Stopped) => {
                        let mut senders = self.senders.lock().unwrap();
                        for sender in senders.values_mut() {
                            //ignore because sockets are closed anyway when we exit
                            let _ =
                                sender.send(DaemonResponse::DaemonEvent(DaemonIpcEvent::Stopped));
                        }
                        sleep(Duration::from_millis(500)); // might not really be necessary but leave time to propagate events
                        exit(0);
                    }
                }
            }
        });
    }

    pub fn install_server(
        &mut self,
        mut event_handler: EventHandler,
        unit_id: String,
        install_path: String,
        unit_file_path: Option<String>,
        server_version: Option<Version>,
        server_type: ServerType,
        accept_eula: bool,
        server_name: Option<String>,
        daemon_queue: Sender<DaemonEvent>,
    ) {
        if self.servers.contains_key(&unit_id) {
            event_handler.raise_event(
                unit_id.as_str(),
                ServerEvent::InstallationFailed {
                    server_id: unit_id.clone(),
                    error: "a unit with that name already exists".to_string(),
                },
            );
        } else {
            spawn(move || {
                let server_id = unit_id.clone();
                let install_result = Daemon::perform_installation(
                    &mut event_handler,
                    unit_id,
                    install_path,
                    unit_file_path.clone(),
                    server_version,
                    server_type,
                    accept_eula,
                    server_name,
                );
                match install_result {
                    Ok(server_unit_config) => {
                        let server_id = server_unit_config.unit.id.clone();
                        daemon_queue
                            .send(DaemonEvent::AddServerUnit {
                                server_unit_config,
                                unit_file: unit_file_path.unwrap().into(),
                            })
                            .expect("send to daemon main event queue");
                        event_handler.raise_event(
                            server_id.as_ref(),
                            ServerEvent::InstallationComplete {
                                server_id: server_id.clone(),
                            },
                        )
                    }
                    Err(e) => event_handler.raise_event(
                        server_id.as_ref(),
                        ServerEvent::InstallationFailed {
                            server_id: server_id.clone(),
                            error: format!("{:?}", e),
                        },
                    ),
                }
            });
        }
    }

    fn perform_installation(
        event_handler: &mut EventHandler,
        unit_id: String,
        install_path: String,
        unit_file_path: Option<String>,
        server_version: Option<Version>,
        server_type: ServerType,
        accept_eula: bool,
        server_name: Option<String>,
    ) -> Result<ServerUnitConfig, InstallError> {
        match server_type {
            ServerType::Paper => {
                let mut paper_installer =
                    PaperServerInstaller::new(event_handler.clone(), unit_id.clone());
                if let Some(unit_file_path) = unit_file_path {
                    let path = Path::new(unit_file_path.as_str());
                    let mut unit_path = PathBuf::new();
                    unit_path.push(path);
                    //TODO check if unit dir is writeable

                    let server_config = paper_installer.install_server(
                        install_path,
                        server_version,
                        accept_eula,
                        server_name,
                    )?;
                    let server_unit_config = ServerUnitConfig {
                        unit: UnitConfig {
                            id: unit_id.clone(),
                            unit_type: "server".to_string(),
                        },
                        server: server_config,
                    };

                    let config_string = toml::to_string(&server_unit_config).unwrap();
                    debug!("writing configuration {} to {:?}", config_string, unit_path);
                    fs::write(unit_path, config_string)
                        .map_err(|e| InstallError::WriteUnitFile(e))?;

                    Ok(server_unit_config)
                } else {
                    //TODO construct path
                    Err(InstallError::DirExists)
                }
            }
            _ => Err(InstallError::UnsupportedServerType(server_type)),
        }
    }

    fn update_server(
        &self,
        mut event_handler: EventHandler,
        unit_id: String,
        server_version: Option<Version>,
        server_type: ServerType,
        unit_config: UnitConfig,
        server_config: ServerConfig,
        unit_file_path: PathBuf,
        daemon_queue: Sender<DaemonEvent>,
    ) {
        spawn(move || {
            let server_id = unit_id.clone();
            let update_result = Daemon::perform_update(
                event_handler.clone(),
                unit_id,
                server_version,
                server_type,
                unit_config,
                server_config,
                unit_file_path.clone(),
            );

            match update_result {
                Ok(server_unit_config) => {
                    let server_id = server_unit_config.unit.id.clone();
                    daemon_queue
                        .send(DaemonEvent::AddServerUnit {
                            server_unit_config,
                            unit_file: unit_file_path.into(),
                        })
                        .expect("send to daemon main event queue");
                    event_handler.raise_event(
                        server_id.as_ref(),
                        ServerEvent::UpdateComplete {
                            server_id: server_id.clone(),
                        },
                    )
                }
                Err(e) => event_handler.raise_event(
                    server_id.as_ref(),
                    ServerEvent::UpdateFailed {
                        server_id: server_id.clone(),
                        error: format!("{:?}", e),
                    },
                ),
            }
        });
    }

    fn perform_update(
        event_handler: EventHandler,
        unit_id: String,
        server_version: Option<Version>,
        server_type: ServerType,
        unit_config: UnitConfig,
        server_config: ServerConfig,
        unit_file_path: PathBuf,
    ) -> Result<ServerUnitConfig, UpdateError> {
        match server_type {
            ServerType::Paper => {
                let mut paper_updater =
                    PaperServerUpdater::new(unit_id, unit_file_path.clone(), event_handler);
                let server_config = paper_updater.update_server(server_version, server_config)?;

                let server_unit_config = ServerUnitConfig {
                    unit: unit_config,
                    server: server_config,
                };

                let config_string = toml::to_string(&server_unit_config).unwrap();
                debug!(
                    "writing configuration {} to {:?}",
                    config_string, unit_file_path
                );
                fs::write(unit_file_path, config_string)
                    .map_err(|e| UpdateError::WriteUnitFile(e))?;

                Ok(server_unit_config)
            }
            _ => Err(UnsupportedServerType(server_type)),
        }
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
                Ok(Some(status)) => {
                    if status.success() {
                        ServerStatus::Down
                    } else {
                        ServerStatus::Errored(status.code())
                    }
                }
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

    pub fn say(&mut self, message: String) {
        self.send_command(format!("say {}", message))
    }

    pub fn stop(&mut self) -> Option<Child> {
        if self.process.is_some() {
            self.send_command("stop".to_string());
            self.process.take()
        } else {
            None
        }
    }

    pub fn has_started(&mut self) -> bool {
        //TODO function isn't nice, but since we are going async we don't need to worry about this currently
        while let ServerStatus::Starting = self.status() {
            sleep(Duration::from_millis(200));
        }
        return true;
    }
}

fn get_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
}
