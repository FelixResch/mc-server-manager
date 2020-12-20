
use clap::{App, SubCommand};
use ipc_channel::ipc::{IpcSender, IpcSelectionResult};
use mcman::ipc::{DaemonCmd, DaemonResponse};
use std::process::exit;
use mcman::get_socket_name;
use term_table::{Table, TableStyle};
use term_table::row::Row;
use term_table::table_cell::TableCell;

fn main() {
    let matches = App::new("mcman")
        .version("0.1.0")
        .about("Interface to the MC Manager Daemon")
        .author("Felix Resch")
        .subcommand(
            SubCommand::with_name("list")
                .about("List currently available units")
        )
        .get_matches();

    if let (cmd, _args) = matches.subcommand() {
        if cmd == "list" {
            let tx = IpcSender::connect(get_socket_name()).unwrap();
            let (remote_tx, rx) = ipc_channel::ipc::channel().unwrap();
            tx.send(DaemonCmd::SetSender(remote_tx)).unwrap();
            tx.send(DaemonCmd::List).unwrap();
            while let Ok(response) = rx.recv() {
                match response {
                    DaemonResponse::List { servers } => {
                        println!("Currently managed servers:");
                        let mut table = Table::new();
                        table.style = TableStyle::simple();

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

                        break
                    }
                    DaemonResponse::UnknownCommand => {
                        eprintln!("Deamon received an unknown command");
                        eprintln!();
                    }
                    DaemonResponse::Version { version} => {
                        println!("Daemon version: {}", version);
                        println!();
                    }
                }
            }
        }
    }
}