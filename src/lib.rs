pub mod ipc;

#[macro_use]
extern crate serde_derive;

use std::fs::File;
use std::path::Path;
use std::io::{Write, Read};
use std::fmt::Display;
use serde::export::Formatter;
use semver::Version;

pub fn get_socket_name() -> String {
    let path = Path::new(".mcman.socket");
    let mut file = File::open(path).unwrap();
    let mut string = String::new();
    file.read_to_string(&mut string);
    return string
}

pub fn set_socket_name(socket_name: &str) {
    let path = Path::new(".mcman.socket");
    let mut file = File::create(&path).unwrap();
    write!(file, "{}", socket_name).unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerStatus {
    Unknown,
    Running,
    Updating,
    Down,
    Lockdown,
}

impl Display for ServerStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub path: String,
    pub server_type: ServerType,
    pub server_version: Version,
    pub server_status: ServerStatus,
}