use crate::daemon::DaemonEvent;
use crate::ipc::{ServerEvent, ServerEventType};
use log::{debug, info};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::spawn;

pub struct EventManager {
    subscriptions: HashMap<(String, ServerEventType), Vec<u32>>,
    cmd_queue: Receiver<EventManagerCmd>,
    daemon_sender: Sender<DaemonEvent>,
}

impl EventManager {
    pub fn new(cmd_queue: Receiver<EventManagerCmd>, daemon_sender: Sender<DaemonEvent>) -> Self {
        Self {
            subscriptions: HashMap::new(),
            cmd_queue,
            daemon_sender,
        }
    }

    pub fn run(mut self) {
        debug!("spawning thread for EventManager");
        spawn(move || {
            debug!("thread for EventManager active");
            while let Ok(cmd) = self.cmd_queue.recv() {
                debug!("incoming EventManager cmd: {:?}", cmd);
                match cmd {
                    EventManagerCmd::DispatchEvent { server_id, event } => {
                        let subscriptions =
                            self.subscriptions.get(&(server_id, event.get_event_type()));
                        if let Some(subscriptions) = subscriptions {
                            for subscription in subscriptions {
                                self.daemon_sender
                                    .send(DaemonEvent::SendEvent {
                                        client_id: *subscription,
                                        event: event.clone(),
                                    })
                                    .unwrap();
                            }
                        }
                    }
                    EventManagerCmd::AddSubscription {
                        server_id,
                        event_type,
                        client_id,
                    } => {
                        let subscriptions = self
                            .subscriptions
                            .entry((server_id, event_type))
                            .or_default();
                        subscriptions.push(client_id);
                    }
                    EventManagerCmd::RemoveSubscription {
                        server_id,
                        event_type,
                        client_id,
                    } => {
                        if let Some(subscriptions) =
                            self.subscriptions.get_mut(&(server_id, event_type))
                        {
                            subscriptions.retain(|id| id == &client_id);
                        }
                    }
                    EventManagerCmd::RemoveAllSubscriptions { client_id } => {
                        self.remove_all_subscriptions(client_id);
                    }
                }
            }
            info!("event handler thread quit unexpectedly")
        });
    }

    fn remove_all_subscriptions(&mut self, client_id: u32) {
        for subscriptions in self.subscriptions.values_mut() {
            subscriptions.retain(|id| id == &client_id);
        }
    }
}

#[derive(Debug)]
pub enum EventManagerCmd {
    DispatchEvent {
        server_id: String,
        event: ServerEvent,
    },
    AddSubscription {
        server_id: String,
        event_type: ServerEventType,
        client_id: u32,
    },
    RemoveSubscription {
        server_id: String,
        event_type: ServerEventType,
        client_id: u32,
    },
    RemoveAllSubscriptions {
        client_id: u32,
    },
}

#[derive(Clone)]
pub struct EventHandler {
    event_dispatcher: Sender<EventManagerCmd>,
}

impl EventHandler {
    pub fn new(event_dispatcher: Sender<EventManagerCmd>) -> Self {
        Self { event_dispatcher }
    }

    pub fn raise_event(&mut self, server_id: &str, event: ServerEvent) {
        debug!("raising event {:?} for unit {}", event, server_id);
        self.event_dispatcher
            .send(EventManagerCmd::DispatchEvent {
                server_id: server_id.to_owned(),
                event,
            })
            .unwrap();
    }
}
