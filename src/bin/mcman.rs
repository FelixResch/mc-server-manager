
use clap::{App, SubCommand, Arg, ArgMatches};
use ipc_channel::ipc::{IpcOneShotServer, IpcSender, IpcReceiver};
use mcman::ipc::{DaemonCmd, DaemonResponse, NewConnection, ServerEvent};
use mcman::fs::get_socket_name;
use term_table::{Table, TableStyle};
use term_table::row::Row;
use term_table::table_cell::TableCell;
use interprocess::local_socket::LocalSocketStream;
use std::io::Write;
use semver::Version;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

fn main() {
    let matches = matches();

    let (cmd, args) = matches.subcommand();
    let mut socket = LocalSocketStream::connect(get_socket_name()).unwrap();

    let (server, path) = IpcOneShotServer::new().unwrap();

    let new_con = NewConnection {
        min_version: None,
        client_version: Version::new(0, 1, 0),
        socket_path: path,
        client_name: "mcman".to_owned(),
    };
    let data = serde_json::to_vec(&new_con).unwrap();

    socket.write_all(&data).unwrap();
    socket.flush().unwrap();
    drop(socket);

    let (res_in, res) = server.accept().unwrap();

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

    let client = Client {
        cmd_out,
        res_in,
    };

    if cmd == "list" {
        client.list();
    } else if cmd == "start" {
        client.start(args);
    } else if cmd == "stop" {
        client.stop(args);
    } else {
        eprintln!("unknown subcommand: {}", cmd);
    }
}

fn matches() -> ArgMatches<'static> {
    App::new("mcman")
        .version("0.1.0")
        .about("Interface to the MC Manager Daemon")
        .author("Felix Resch")
        .subcommand(
            SubCommand::with_name("list")
                .about("List currently available units")
        )
        .subcommand(
            SubCommand::with_name("start")
                .about("Start a server")
                .arg(Arg::with_name("server_name")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("no-wait")
                    .required(true)
                    .takes_value(false)
                    .long("no-wait"))
        )
        .subcommand(
            SubCommand::with_name("stop")
                .about("Stop a server")
                .arg(Arg::with_name("server_name")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("no-wait")
                    .required(true)
                    .takes_value(false)
                    .long("no-wait"))
        )
        .get_matches()
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
                    table.add_row(
                        Row::new(vec![
                            TableCell::new(&server.name),
                            TableCell::new(&server.path),
                            TableCell::new(&server.server_type),
                            TableCell::new(&server.server_version),
                            TableCell::new(&server.server_status),
                        ])
                    )
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
        let server_name = args.unwrap().value_of("server_name").unwrap();
        let no_wait = args.unwrap().is_present("no-wait");
        self.cmd_out.send(DaemonCmd::Start {
            server_id: server_name.to_owned(),
            wait: !no_wait,
        }).unwrap();
        if no_wait {
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerStarted { server_id: server_name } = response {
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
                if let DaemonResponse::ServerEvent { event} = response {
                    if let ServerEvent::ServerStarting { server_id } = event {
                        spinner.set_message(format!("Starting {}", server_id).as_str());
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
                if let DaemonResponse::ServerEvent { event} = response {
                    if let ServerEvent::ServerStarted { server_id } = event {
                        spinner.finish_with_message(format!("Started {}", server_id).as_str());
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
        let server_name = args.unwrap().value_of("server_name").unwrap();
        let no_wait = args.unwrap().is_present("no-wait");
        self.cmd_out.send(DaemonCmd::Stop {
            server_id: server_name.to_owned(),
            wait: !no_wait,
        }).unwrap();
        if no_wait {
            if let Ok(response) = self.res_in.recv() {
                if let DaemonResponse::ServerStopped { server_id: server_name } = response {
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
                if let DaemonResponse::ServerEvent { event} = response {
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
                if let DaemonResponse::ServerEvent { event} = response {
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
}