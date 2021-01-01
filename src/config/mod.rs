//! Contains structs for loading and modifying daemon and server configurations.
use crate::daemon::paper::PaperServer;
use crate::daemon::Server;
use semver::Version;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Config of a daemon
#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// List of server configurations
    pub servers: Vec<ServerConfig>,
    /// List of units to start at the start of the daemon.
    pub autostart: Vec<String>,
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
    /// The id of the server
    pub id: String,
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
        let mut map: HashMap<_, Box<dyn Server + Send>> =
            HashMap::with_capacity(self.servers.len());
        for server in &self.servers {
            match server.type_name.as_ref() {
                "paper" => {
                    map.insert(
                        server.id.clone(),
                        Box::new(PaperServer::create(server.clone())),
                    );
                }
                other => {
                    println!("unsupported server type: {}", other)
                }
            }
        }
        map
    }
}
