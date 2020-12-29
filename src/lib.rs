pub mod ipc;
pub mod config;
pub mod daemon;

#[macro_use]
extern crate serde_derive;

use std::fmt::Display;
use serde::export::Formatter;
use semver::Version;

pub mod fs {
    use std::fs::{File, remove_file};
    use std::path::Path;
    use std::io::{Write, Read};

    pub fn get_socket_name() -> String {
        let path = Path::new(".mcman.socket");
        let mut file = File::open(path).unwrap();
        let mut string = String::new();
        file.read_to_string(&mut string).unwrap();
        return string
    }

    pub fn set_socket_name(socket_name: &str) {
        let path = Path::new(".mcman.socket");
        let mut file = File::create(&path).unwrap();
        write!(file, "{}", socket_name).unwrap();
    }

    pub fn clear_socket_name() {
        let path = Path::new(".mcman.socket");
        remove_file(path).unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerType {
    Vanilla,
    Paper,
    Bukkit,
    Spigot,
}

impl Display for ServerType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerStatus {
    Unknown,
    Starting,
    Running,
    Updating,
    Down,
    Lockdown,
    Errored(Option<i32>),
    Stopping,
}

impl Display for ServerStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub path: String,
    pub server_type: ServerType,
    pub server_version: Version,
    pub server_status: ServerStatus,
}