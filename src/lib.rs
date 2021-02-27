//! Library to manage minecraft servers (currently only paper servers).
//! This library contains components used in the daemon and the control applications.

#![warn(clippy::unwrap_used)]
#![forbid(clippy::missing_docs_in_private_items)]

pub mod config;
pub mod daemon;
pub mod ipc;
pub mod repo;

#[macro_use]
extern crate serde_derive;

use crate::config::UnitConfig;
use semver::Version;
use serde::export::Formatter;
use std::fmt::Display;
use std::path::PathBuf;

/// Contains helper methods for IPC methods.
//TODO maybe move to ipc module and change functionality of methods to use envs
pub mod files {
    use std::fs::{remove_file, File};
    use std::io::{Read, Write};
    use std::path::Path;

    /// Return the name of the local socket used by the daemon.
    pub fn get_socket_name() -> String {
        let path = Path::new(".mcman.socket");
        let mut file = File::open(path).expect("open socket configuration file");
        let mut string = String::new();
        file.read_to_string(&mut string)
            .expect("read socket configuration file");
        string
    }

    /// Set the socket name used by the daemon.
    /// This method should only be used by the/a daemon.
    pub fn set_socket_name(socket_name: &str) {
        let path = Path::new(".mcman.socket");
        let mut file = File::create(&path).expect("create socket configuration file");
        write!(file, "{}", socket_name).expect("write configuration to socket configuration file");
    }

    /// Delete the file where the name of the local socket is stored.
    pub fn clear_socket_name() {
        let path = Path::new(".mcman.socket");
        let _ = remove_file(path);
    }
}

/// Types of servers (in the future) supported by the daemon.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerType {
    /// Vanilla Minecraft server
    Vanilla,
    /// PaperMC server
    Paper,
    /// Bukkit server
    Bukkit,
    /// Spigot server
    Spigot,
}

impl Display for ServerType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Status of a server.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerStatus {
    /// The status of the server is currently unknown
    Unknown,
    /// The server is currently starting
    Starting,
    /// The server has started is currently running
    Running,
    /// The server is currently updating
    Updating,
    /// The server is not running (stopped)
    Down,
    /// The server is in lockdown.
    ///
    /// If possible (in the future) the process in which the server is running is suspended and a backup of the worlds in made.
    Lockdown,
    /// The process of the server has stopped.
    /// The error code is contained in this variant.
    Errored(Option<i32>),
    /// The server is shutting down.
    Stopping,
}

impl Display for ServerStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Info about a server
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerInfo {
    /// The name/id of a server
    pub name: String,
    /// The path to the server directory
    pub path: String,
    /// The type of the server (server application)
    pub server_type: ServerType,
    /// The version of the server software
    pub server_version: Version,
    /// The current status of the server
    pub server_status: ServerStatus,
}

/// General properties of any unit.
pub trait Unit {
    /// The file which is used to store the config of this unit.
    fn unit_file_path(&self) -> PathBuf;
    /// The unit config for this unit.
    fn unit_config(&self) -> UnitConfig;
}
