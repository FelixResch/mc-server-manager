use semver::Version;
use std::path::Path;
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;
use crate::daemon::Server;
use crate::daemon::paper::PaperServer;

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub servers: Vec<ServerConfig>,
    pub autostart: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub name: String,
    pub path: Box<Path>,
    #[serde(rename="type")]
    pub type_name: String,
    pub jar: String,
    pub version: Version,
    pub id: String,
}

impl DaemonConfig {

    pub fn load(path: &Path) -> DaemonConfig {
        let mut file = File::open(path).unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        toml::from_str(&content).unwrap()
    }

    pub fn create_servers(&self) -> HashMap<String, Box<dyn Server + Send>> {
        let mut map: HashMap<_, Box<dyn Server + Send>> = HashMap::with_capacity(self.servers.len());
        for server in &self.servers {
            match server.type_name.as_ref() {
                "paper" => {
                    map.insert(server.id.clone(), Box::new(PaperServer::create(server.clone())));
                }
                other => {
                    println!("unsupported server type: {}", other)
                }
            }
        }
        map
    }
}