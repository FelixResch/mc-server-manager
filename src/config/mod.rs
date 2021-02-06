//! Contains structs for loading and modifying daemon and server configurations.
use crate::daemon::paper::PaperServer;
use crate::daemon::Server;
use log::{debug, error, info, warn};
use semver::Version;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{read_to_string, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, Error, WalkDir};

/// Config of a daemon
#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// List of server configurations
    pub unit_directories: Vec<String>,
    /// List of directories to look for units in.
    pub autostart: Vec<String>,
    /// Path to the socket that is used in IPC communication.
    ///
    /// If this parameter is a relative path, make sure, that daemon and client are run
    /// in the same directory!
    pub socket_file: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnitConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub unit_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimpleUnitConfig {
    pub unit: UnitConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerUnitConfig {
    pub unit: UnitConfig,
    pub server: ServerConfig,
}

/// Config of a server
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    /// The name of the server
    pub name: String,
    /// The path where a server is located
    pub path: Box<Path>,
    /// The name of a server type
    #[serde(rename = "type")]
    pub type_name: String,
    /// The location of the jar relative to `path`
    pub jar: String,
    /// The version of the installed server software
    pub version: Version,
    /// The amount of memory dedicated to a server in gigabyte
    pub memory: u32,
}

impl DaemonConfig {
    /// Load the configuration from the given path
    pub fn load(path: &Path) -> DaemonConfig {
        let mut file = File::open(path).unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        toml::from_str(&content).unwrap()
    }

    /// Create server instances from the loaded config.
    pub fn create_servers(&self) -> HashMap<String, Box<dyn Server + Send>> {
        let mut map: HashMap<_, Box<dyn Server + Send>> = HashMap::new();
        for unit_dir in &self.unit_directories {
            for entry in WalkDir::new(unit_dir) {
                match entry {
                    Ok(entry) => {
                        let entry_path = entry.path();
                        if entry_path.is_file() {
                            if let Some(ext) = entry_path.extension() {
                                if ext.to_string_lossy() == "toml" {
                                    info!("found unit file: {:?}", entry_path);
                                    let file_content = read_to_string(entry_path).unwrap();
                                    let simple_unit_config: SimpleUnitConfig =
                                        toml::from_str(file_content.as_str()).unwrap();
                                    info!(
                                        "found unit {} with type {}",
                                        simple_unit_config.unit.id,
                                        simple_unit_config.unit.unit_type
                                    );
                                    debug!("loaded unit config {:?}", simple_unit_config);
                                    if simple_unit_config.unit.unit_type == "server" {
                                        let server_unit_config: ServerUnitConfig =
                                            toml::from_str(file_content.as_str()).unwrap();
                                        let unit_id = server_unit_config.unit.id.clone();
                                        debug!(
                                            "loaded server unit config {:?}",
                                            server_unit_config
                                        );
                                        let server = crate::daemon::create_server(
                                            server_unit_config,
                                            entry_path.to_path_buf(),
                                        );
                                        if let Ok(server) = server {
                                            map.insert(unit_id, server);
                                        }
                                    } else {
                                        debug!("unit is not a server {:?}", entry_path);
                                    }
                                } else {
                                    debug!(
                                        "unknown extension {} for unit file {:?}",
                                        ext.to_string_lossy(),
                                        entry_path
                                    );
                                }
                            } else {
                                debug!("file has no extension {:?}", entry_path);
                            }
                        } else {
                            debug!("skipping unit path {:?}", entry_path);
                        }
                    }
                    Err(e) => {
                        warn!("could not load unit file {}", e);
                    }
                }
            }
        }
        map
    }
}
