//! This module contains structs and traits for event management in the daemon.
//!
//! The events should be reduced to events, which could be associated with a server, but the
//! framework is generic enough to also allow daemon events.

use crate::daemon::DaemonEvent;
use crate::ipc::{ServerEvent, ServerEventType};
use log::{debug, info};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::spawn;

/// Manages subscriptions and the dispatch of events.
pub struct EventManager {
    /// The subscriptions which are currently handled
    subscriptions: HashMap<(String, ServerEventType), Vec<u32>>,
    /// Incoming queue for events
    cmd_queue: Receiver<EventManagerCmd>,
    /// Sender to daemon (used for sending to clients)
    daemon_sender: Sender<DaemonEvent>,
}

impl EventManager {
    /// Creates a new EventManager
    pub fn new(cmd_queue: Receiver<EventManagerCmd>, daemon_sender: Sender<DaemonEvent>) -> Self {
        Self {
            subscriptions: HashMap::new(),
            cmd_queue,
            daemon_sender,
        }
    }

    /// Spawns a new thread and dispatches incoming events.
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
                                    .expect("send to daemon main event queue");
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

    /// Removes all subscriptions for a given client.
    fn remove_all_subscriptions(&mut self, client_id: u32) {
        for subscriptions in self.subscriptions.values_mut() {
            subscriptions.retain(|id| id == &client_id);
        }
    }
}

/// Commands to control an [`EventManager`]
#[derive(Debug)]
pub enum EventManagerCmd {
    /// Dispatch an event received from server identified by `server_id`.
    DispatchEvent {
        /// The server that generated that event
        server_id: String,
        /// The event that has been generated
        event: ServerEvent,
    },
    /// Client with the given client id wants to listen for events of type `event_type` on server `server_id`.
    AddSubscription {
        /// The server on which the subscription should be added
        server_id: String,
        /// The type of event, the subscription should listen for
        event_type: ServerEventType,
        /// The client which should be notified about events
        client_id: u32,
    },
    /// Remove a subscription of a client for a specific server and event type
    RemoveSubscription {
        /// The server on which the listening should be stopped
        server_id: String,
        /// The type of event, the subscription should be cancelled
        event_type: ServerEventType,
        /// The client which wants to remove the subscription
        client_id: u32,
    },
    /// Remove all subscriptions of a given client
    RemoveAllSubscriptions {
        /// The client for which all subscriptions should be removed
        client_id: u32,
    },
}

/// Handler for events.
#[derive(Clone)]
pub struct EventHandler {
    /// Sender to the event manager thread
    event_dispatcher: Sender<EventManagerCmd>,
}

impl EventHandler {
    /// Creates a new [`EventManager`] with the given Sender
    pub fn new(event_dispatcher: Sender<EventManagerCmd>) -> Self {
        Self { event_dispatcher }
    }

    /// Raise an event.
    /// The event is encapsulated into an [`EventManagerCmd`] and sent over the sender to the event manager thread.
    pub fn raise_event(&mut self, server_id: &str, event: ServerEvent) {
        debug!("raising event {:?} for unit {}", event, server_id);
        self.event_dispatcher
            .send(EventManagerCmd::DispatchEvent {
                server_id: server_id.to_owned(),
                event,
            })
            .expect("send to event manager queue");
    }
}
