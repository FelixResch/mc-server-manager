use crate::daemon::event::EventHandler;
use crate::daemon::{LogService, OutputState};
use crate::ipc::ServerEvent;
use log::info;
use std::fs::{create_dir_all, File};
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::process::ChildStdout;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread::spawn;

pub struct BasicLogService {
    event_handler: EventHandler,
}

impl BasicLogService {
    pub fn new(event_handler: EventHandler) -> Self {
        Self { event_handler }
    }
}

struct BasicLogServiceHandler {
    state: Arc<RwLock<OutputState>>,
    out: ChildStdout,
    event_handler: EventHandler,
    server_id: String,
}

impl LogService for BasicLogService {
    fn manage_output(&mut self, out: ChildStdout, server_id: String) -> Arc<RwLock<OutputState>> {
        let state = Arc::new(RwLock::new(OutputState::Unknown));
        let handler = BasicLogServiceHandler {
            state: state.clone(),
            out,
            event_handler: self.event_handler.clone(),
            server_id,
        };
        handler.run();
        state
    }
}

impl BasicLogServiceHandler {
    fn run(self) {
        spawn(move || {
            let Self {
                state,
                out,
                mut event_handler,
                server_id,
            } = self;
            let reader = BufReader::new(out);

            let mut out_path = PathBuf::new();
            out_path.push("log");
            out_path.push(&server_id);
            out_path.push(format!("{}_out.log", chrono::Utc::now()));
            create_dir_all(out_path.parent().unwrap()).unwrap();
            let mut writer = BufWriter::new(File::create(out_path).unwrap());

            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        writeln!(writer, "{}", line).unwrap();
                        if line.starts_with("Loading libraries") {
                            info!("server {} starting", server_id);
                            *state.write().unwrap().deref_mut() = OutputState::Starting;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStarting {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Done (") && line.contains("s)! For help") {
                            info!("server {} started", server_id);
                            *state.write().unwrap().deref_mut() = OutputState::Started;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStarted {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Stopping the server") {
                            info!("server {} stopping", server_id);
                            *state.write().unwrap().deref_mut() = OutputState::Stopping;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStopping {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Closing Server") {
                            info!("server {} stopped", server_id);
                            *state.write().unwrap().deref_mut() = OutputState::Stopped;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStopped {
                                    server_id: server_id.clone(),
                                },
                            )
                        }
                    }
                    _ => {
                        break;
                    }
                }
            }
        });
    }
}
