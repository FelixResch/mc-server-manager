//! This module contains a simple implementation for a log service.

use crate::daemon::event::EventHandler;
use crate::daemon::{LogService, OutputState};
use crate::ipc::ServerEvent;
use log::{info, warn};
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

/// Basic implementation for a [`LogService`].
/// It parses the Output of a process to determine the current state of a server process.
///
/// The output is then redirected to `log/<unit_name>/<time_and_date>_out.log`.
pub struct BasicLogService {
    /// The event handler for detected server events
    event_handler: EventHandler,
}

impl BasicLogService {
    /// Creates a new log service with the given [`EventHandler`].
    pub fn new(event_handler: EventHandler) -> Self {
        Self { event_handler }
    }
}

/// Handles the output of a single process.
struct BasicLogServiceHandler {
    /// The current state of the process as determined by parsing the process output.
    state: Arc<RwLock<OutputState>>,
    /// The output of the child process
    out: ChildStdout,
    /// The [`EventHandler`] to which the events should be passed
    event_handler: EventHandler,
    /// The id of the server this service is logging for
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
    /// Spawns a thread which parses the output.
    /// This is one of the places which could be rewritten using asynchronous tools.
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
            create_dir_all(out_path.parent().expect("get parent path"))
                .expect("create dirs to log directory");
            let mut writer = BufWriter::new(File::create(out_path).expect("create output file"));

            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        writeln!(writer, "{}", line).unwrap();
                        if line.starts_with("Loading libraries") {
                            info!("server {} starting", server_id);
                            *state.write().expect("lock rwlock for write").deref_mut() =
                                OutputState::Starting;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStarting {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Done (") && line.contains("s)! For help") {
                            info!("server {} started", server_id);
                            *state.write().expect("lock rwlock for write").deref_mut() =
                                OutputState::Started;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStarted {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Stopping the server") {
                            info!("server {} stopping", server_id);
                            *state.write().expect("lock rwlock for write").deref_mut() =
                                OutputState::Stopping;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStopping {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Closing Server") {
                            info!("server {} stopped", server_id);
                            *state.write().expect("lock rwlock for write").deref_mut() =
                                OutputState::Stopped;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerStopped {
                                    server_id: server_id.clone(),
                                },
                            )
                        } else if line.contains("Failed to load eula.txt") {
                            warn!("eula not accepted for unit {}", server_id);
                            *state.write().expect("lock rwlock for write").deref_mut() =
                                OutputState::Errored;
                            event_handler.raise_event(
                                &server_id,
                                ServerEvent::ServerFailed {
                                    server_id: server_id.clone(),
                                    error: "EULA not accepted".to_string(),
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
