#![feature(stmt_expr_attributes)]

use clap::{App, Arg, ArgMatches, SubCommand};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use interprocess::local_socket::LocalSocketStream;
use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use mcman::config::DaemonConfig;
use mcman::files::get_socket_name;
use mcman::ipc::{DaemonCmd, DaemonResponse, NewConnection, ServerEvent, DaemonIpcEvent};
use mcman::ServerType;
use regex::Regex;
use semver::{Identifier, Version};
use std::error::Error;
use std::io::Write;
#[cfg(not(debug_assertions))]
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::exit;
use term_table::row::Row;
use term_table::table_cell::TableCell;
use term_table::{Table, TableStyle};

fn main() {
    let matches = matches();
    //println!("parsed arguments");
    let (cmd, args) = matches.subcommand();
    //println!("subcommand {}, {:?}", cmd, args);
    let server = match send_connection_request() {
        Ok(server) => server,
        Err(e) => {
            eprintln!("error when connecting to daemon: {}", e);
            eprintln!();
            eprintln!("make sure the daemon is running and the client is correctly configured!");
            exit(1);
        }
    };

    let (res_in, res) = server.accept().unwrap();
    //println!("accepted incoming connection");

    let cmd_out = if let DaemonResponse::SetSender { sender } = res {
        sender
    } else {
        panic!()
    };

    if let Ok(response) = res_in.recv() {
        if let DaemonResponse::Version { version } = response {
            println!("Daemon version: {}", version)
        }
    } else {
        panic!()
    }

    let client = Client { cmd_out, res_in };

    if cmd == "list" {
        client.list();
    } else if cmd == "start" {
        client.start(args);
    } else if cmd == "stop" {
        client.stop(args);
    } else if cmd == "install" {
        client.install(args);
    } else if cmd == "update" {
        client.update(args);
    } else if cmd == "stop-daemon" {
        client.stop_daemon(args);
    } else {
        eprintln!("unknown subcommand: {}", cmd);
    }
}

fn matches() -> ArgMatches<'static> {
    App::new("mcman")
        .version("0.1.0")
        .about("Interface to the MC Manager Daemon")
        .author("Felix Resch")
        .subcommand(SubCommand::with_name("list").about("List currently available units"))
        .subcommand(
            SubCommand::with_name("start")
                .about("Start a server")
                .arg(
                    Arg::with_name("server-id")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("no-wait")
                        .takes_value(false)
                        .long("no-wait"),
                ),
        )
        .subcommand(
            SubCommand::with_name("stop")
                .about("Stop a server")
                .arg(
                    Arg::with_name("server-id")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("no-wait")
                        .takes_value(false)
                        .long("no-wait"),
                ),
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Install a new server")
                .arg(
                    Arg::with_name("unit-id")
                        .help("The unit id of the new server")
                        .short("u")
                        .long("unit-id")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("install-path")
                        .help("The server directory")
                        .long("install-path")
                        .short("i")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("unit-file-path")
                        .help("The name of the unit file. If left empty any unit directory in which the current user can write into is chosen")
                        .long("unit-file-path")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("server-version")
                        .help("The version of the server you want to install. (Builds are denoted by '+<build_no>')")
                        .long("server-version")
                        .short("v")
                        .takes_value(true)
                        .required(false)
                        .validator(|str| {
                            let regex = Regex::new("^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)(?:-((?:0|[1-9]\\d*|\\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\\.(?:0|[1-9]\\d*|\\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\\+([0-9a-zA-Z-]+(?:\\.[0-9a-zA-Z-]+)*))?$").unwrap();
                            if regex.is_match(str.as_str()) {
                                Ok(())
                            } else {
                                Err("version string does not match pattern".to_string())
                            }
                        }),
                )
                .arg(
                    Arg::with_name("server-type")
                        .help("The type of server you want to install")
                        .long("server-type")
                        .short("t")
                        .takes_value(true)
                        .required(true)
                        .possible_value("paper"),
                ).arg(
                Arg::with_name("eula")
                    .help("Writes the accept eula file to the installation directory. Only set this option, if you have read the EULA.")
                    .long("eula")
                    .short("e")
                )
                .arg(
                    Arg::with_name("server-name")
                        .long("server-name")
                        .help("The name that should be displayed in the Minecraft multiplayer server list")
                        .takes_value(true)
                        .required(false),
                )
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update an already existing server")
                .arg(
                    Arg::with_name("unit-id")
                        .help("The unit id of the new server")
                        .short("u")
                        .long("unit-id")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("server-version")
                        .help("The version of the server you want to install. (Builds are denoted by '+<build_no>')")
                        .long("server-version")
                        .short("v")
                        .takes_value(true)
                        .required(false)
                        .validator(|str| {
                            let regex = Regex::new("^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)(?:-((?:0|[1-9]\\d*|\\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\\.(?:0|[1-9]\\d*|\\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\\+([0-9a-zA-Z-]+(?:\\.[0-9a-zA-Z-]+)*))?$").unwrap();
                            if regex.is_match(str.as_str()) {
                                Ok(())
                            } else {
                                Err("version string does not match pattern".to_string())
                            }
                        }),
                )
        )
        .subcommand(
            SubCommand::with_name("stop-daemon")
                .about("Shut down the minecraft server manager daemon")
        )
        .get_matches()
}

#[inline(never)]
fn send_connection_request() -> Result<IpcOneShotServer<DaemonResponse>, Box<dyn Error>> {
    let config = DaemonConfig::load(Path::new("mcman.toml"));
    let socket_name = config.socket_file;

    let mut socket = LocalSocketStream::connect(socket_name)?;

    let (server, path) = IpcOneShotServer::new()?;
    //println!("created OneShotServer at {}", path);

    let new_con = NewConnection {
        min_version: None,
        client_version: Version::new(0, 1, 0),
        socket_path: path,
        client_name: "mcman".to_owned(),
    };
    //println!("created NewConnection struct: {:?}", new_con);
    let data = serde_json::to_vec(&new_con)?;
    //println!("encoded NewConnection struct {}", String::from_utf8_lossy(&data));

    //println!("needs drop: {:?}", needs_drop::<LocalSocketStream>());
    //println!("wrote data: {:?}", socket.write_all(&data));
    socket.write_all(&data)?;
    #[cfg(not(debug_assertions))]
    unsafe {
        /*
        In nightly 1.50.0 socket is not dropped in release builds, therefore a manual closing of the file descriptor
        is necessary.
         */
        libc::close(socket.as_raw_fd());
    }
    Ok(server)
}

struct Client {
    cmd_out: IpcSender<DaemonCmd>,
    res_in: IpcReceiver<DaemonResponse>,
}

impl Client {
    fn recv_other(&self, response: DaemonResponse) {
        panic!("unexpected response at this time: {:?}", response);
    }

    fn list(&self) {
        self.cmd_out.send(DaemonCmd::List).unwrap();

        if let Ok(response) = self.res_in.recv() {
            if let DaemonResponse::List { servers } = response {
                println!("Currently managed servers:");
                let mut table = Table::new();
                table.style = TableStyle::rounded();

                for server in servers {
                    table.add_row(Row::new(vec![
                        TableCell::new(&server.name),
                        TableCell::new(&server.path),
                        TableCell::new(&server.server_type),
                        TableCell::new(&server.server_version),
                        TableCell::new(&server.server_status),
                    ]))
                }

                println!("{}", table.render());
            } else {
                self.recv_other(response);
            }
        } else {
            panic!()
        }
    }

    fn start(&self, args: Option<&ArgMatches>) {
        let server_name = args.unwrap().value_of("server-id").unwrap();
        let no_wait = args.unwrap().is_present("no-wait");
        self.cmd_out
            .send(DaemonCmd::Start {
                server_id: server_name.to_owned(),
                wait: !no_wait,
            })
            .unwrap();
        if no_wait {
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerStarted {
                    server_id: server_name,
                } = response
                {
                    println!("Started {}", server_name);
                } else {
                    self.recv_other(response);
                }
            } else {
                panic!()
            }
        } else {
            let spinner = ProgressBar::new_spinner()
                .with_style(ProgressStyle::default_spinner().tick_chars("⣷⣯⣟⡿⢿⣻⣽⣾✓"));
            spinner.set_draw_target(ProgressDrawTarget::stdout());
            spinner.set_message("Waiting for daemon response");
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::Ok = response {
                    spinner.set_message("Waiting for server to react to command");
                } else {
                    self.recv_other(response)
                }
            } else {
                panic!()
            }
            spinner.enable_steady_tick(100);
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerEvent { event } = response {
                    if let ServerEvent::ServerStarting { server_id } = event {
                        spinner.set_message(format!("Starting {}", server_id).as_str());
                    } else if let ServerEvent::ServerFailed { server_id, error} = event {
                        spinner.finish_and_clear();
                        spinner.println(format!("Starting unit {} failed: {}", server_id, error))
                    } else {
                        panic!()
                    }
                } else {
                    self.recv_other(response)
                }
            } else {
                panic!()
            }
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerEvent { event } = response {
                    if let ServerEvent::ServerStarted { server_id } = event {
                        spinner.finish_with_message(format!("Started {}", server_id).as_str());
                    } else if let ServerEvent::ServerFailed { server_id, error} = event {
                        spinner.finish_and_clear();
                        spinner.println(format!("Starting unit {} failed: {}", server_id, error))
                    } else {
                        panic!()
                    }
                } else {
                    self.recv_other(response)
                }
            } else {
                panic!()
            }
        }
    }

    fn stop(&self, args: Option<&ArgMatches>) {
        let server_name = args.unwrap().value_of("server-id").unwrap();
        let no_wait = args.unwrap().is_present("no-wait");
        self.cmd_out
            .send(DaemonCmd::Stop {
                server_id: server_name.to_owned(),
                wait: !no_wait,
            })
            .unwrap();
        if no_wait {
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerStopped {
                    server_id: server_name,
                } = response
                {
                    println!("Stopped {}", server_name);
                } else {
                    self.recv_other(response);
                }
            } else {
                panic!()
            }
        } else {
            let spinner = ProgressBar::new_spinner()
                .with_style(ProgressStyle::default_spinner().tick_chars("⣷⣯⣟⡿⢿⣻⣽⣾✓"));
            spinner.set_draw_target(ProgressDrawTarget::stdout());
            spinner.set_message("Waiting for daemon response");
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::Ok = response {
                    spinner.set_message("Waiting for server to react to command");
                } else {
                    self.recv_other(response)
                }
            } else {
                panic!()
            }
            spinner.enable_steady_tick(100);
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerEvent { event } = response {
                    if let ServerEvent::ServerStopping { server_id } = event {
                        spinner.set_message(format!("Stopping {}", server_id).as_str());
                    } else {
                        panic!()
                    }
                } else {
                    self.recv_other(response)
                }
            } else {
                panic!()
            }
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerEvent { event } = response {
                    if let ServerEvent::ServerStopped { server_id } = event {
                        spinner.finish_with_message(format!("Stopped {}", server_id).as_str());
                    } else {
                        panic!()
                    }
                } else {
                    self.recv_other(response)
                }
            } else {
                panic!()
            }
        }
    }

    pub fn install(&self, args: Option<&ArgMatches>) {
        let args = args.unwrap();
        let version = match args.value_of("server-version") {
            Some(version) => Version::parse(version).ok(),
            None => None,
        };
        let unit_id = args.value_of("unit-id").unwrap().to_string();
        let install_path = args.value_of("install-path").unwrap().to_string();
        let unit_file_path = args.value_of("unit-file-path").map(|str| str.to_string());

        let eula = args.is_present("eula");
        let server_type = match args.value_of("server-type").unwrap() {
            "paper" => ServerType::Paper,
            _ => panic!("unknown server type"),
        };

        let server_name = args.value_of("server-name").map(|str| str.to_string());

        self.cmd_out.send(DaemonCmd::InstallServer {
            unit_id,
            install_path,
            unit_file_path,
            server_version: version,
            server_type,
            accept_eula: eula,
            server_name,
        });

        let spinner = ProgressBar::new_spinner()
            .with_style(ProgressStyle::default_spinner().tick_chars("⣷⣯⣟⡿⢿⣻⣽⣾✓"));

        spinner.set_draw_target(ProgressDrawTarget::stdout());
        spinner.set_message("Waiting for daemon");
        spinner.enable_steady_tick(100);

        if let (Ok(DaemonResponse::Ok)) = self.res_in.recv() {
            spinner.set_message("Starting installation")
        }

        while let Ok(DaemonResponse::ServerEvent { event }) = self.res_in.recv() {
            match event {
                ServerEvent::ActionProgress {
                    server_id,
                    action,
                    progress: _progress,
                    maximum: _maximum,
                    action_number: _action_number,
                } => spinner.set_message(format!("[{}] {}", server_id, action).as_str()),
                ServerEvent::InstallationComplete { server_id } => {
                    spinner.finish_with_message(format!("[DONE] installed {}", server_id).as_str());
                    break;
                }
                ServerEvent::InstallationFailed { server_id, error } => {
                    spinner.abandon_with_message(
                        format!("[ERROR] error while installing {}: {:?}", server_id, error)
                            .as_str(),
                    );
                    break;
                }
                _ => (),
            }
        }
    }

    pub fn update(&self, args: Option<&ArgMatches>) {
        let args = args.unwrap();
        let version = match args.value_of("server-version") {
            Some(version) => Version::parse(version).ok(),
            None => None,
        };
        let unit_id = args.value_of("unit-id").unwrap().to_string();

        self.cmd_out.send(DaemonCmd::UpdateServer {
            unit_id,
            server_version: version,
        });

        let spinner = ProgressBar::new_spinner()
            .with_style(ProgressStyle::default_spinner().tick_chars("⣷⣯⣟⡿⢿⣻⣽⣾✓"));

        spinner.set_draw_target(ProgressDrawTarget::stdout());
        spinner.set_message("Waiting for daemon");
        spinner.enable_steady_tick(100);

        if let (Ok(DaemonResponse::Ok)) = self.res_in.recv() {
            spinner.set_message("Starting update")
        }

        while let Ok(DaemonResponse::ServerEvent { event }) = self.res_in.recv() {
            match event {
                ServerEvent::ActionProgress {
                    server_id,
                    action,
                    progress: _progress,
                    maximum: _maximum,
                    action_number: _action_number,
                } => spinner.set_message(format!("[{}] {}", server_id, action).as_str()),
                ServerEvent::UpdateComplete { server_id } => {
                    spinner.finish_with_message(format!("[DONE] installed {}", server_id).as_str());
                    break;
                }
                ServerEvent::UpdateFailed { server_id, error } => {
                    spinner.abandon_with_message(
                        format!("[ERROR] error while installing {}: {:?}", server_id, error)
                            .as_str(),
                    );
                    break;
                }
                _ => (),
            }
        }
    }

    pub fn stop_daemon(&self, args: Option<&ArgMatches>) {
        self.cmd_out.send(DaemonCmd::StopDaemon).unwrap();

        let spinner = ProgressBar::new_spinner()
            .with_style(ProgressStyle::default_spinner().tick_chars("⣷⣯⣟⡿⢿⣻⣽⣾✓"));

        spinner.set_draw_target(ProgressDrawTarget::stdout());
        spinner.set_message("Waiting for daemon");
        spinner.enable_steady_tick(100);

        if let (Ok(DaemonResponse::Ok)) = self.res_in.recv() {
            spinner.set_message("Daemon shutdown in progress")
        }

        if let (Ok(DaemonResponse::DaemonEvent(DaemonIpcEvent::Stopped))) = self.res_in.recv() {
            spinner.finish_with_message("Daemon has stopped")
        }
    }
}
