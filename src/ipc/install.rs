mod iops;

use crate::config::ServerConfig;
use crate::daemon::event::EventHandler;
use crate::daemon::Server;
use crate::ipc::DaemonCmd::InstallServer;
use crate::ipc::ServerEvent;
use crate::repo::paper::PaperRepository;
use crate::repo::Repository;
use crate::ServerType;
use semver::Version;
use std::error::Error;
use std::fs::{self, create_dir_all, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub trait ServerInstaller {
    fn install_server(
        &mut self,
        install_path: String,
        server_version: Option<Version>,
        eula: bool,
        server_name: Option<String>,
    ) -> Result<ServerConfig, InstallError>;
}

#[derive(Debug)]
pub enum InstallError {
    DirExists,
    CreateDir(io::Error),
    DownloadFailed(Box<dyn Error + 'static + Send>),
    WriteInitialSettings(io::Error),
    PerformPatch(String),
    WriteUnitFile(io::Error),
    UnsupportedServerType(ServerType),
    UnitAlreadyExists,
}

pub struct PaperServerInstaller {
    event_handler: EventHandler,
    unit_id: String,
}

impl PaperServerInstaller {
    pub fn new(event_handler: EventHandler, unit_id: String) -> PaperServerInstaller {
        Self {
            event_handler,
            unit_id,
        }
    }
}

impl ServerInstaller for PaperServerInstaller {
    fn install_server(
        &mut self,
        install_path: String,
        server_version: Option<Version>,
        eula: bool,
        server_name: Option<String>,
    ) -> Result<ServerConfig, InstallError> {
        let path = Path::new(install_path.as_str());
        if path.exists() {
            Err(InstallError::DirExists)?;
        }

        self.event_handler.raise_event(
            self.unit_id.as_str(),
            ServerEvent::ActionProgress {
                server_id: self.unit_id.clone(),
                action: "creating server directory".to_string(),
                progress: None,
                maximum: None,
                action_number: 1,
            },
        );

        create_dir_all(path).map_err(|e| InstallError::CreateDir(e))?;

        //TODO symlink cache here

        if eula {
            self.event_handler.raise_event(
                self.unit_id.as_str(),
                ServerEvent::ActionProgress {
                    server_id: self.unit_id.clone(),
                    action: "creating initial server configuration".to_string(),
                    progress: None,
                    maximum: None,
                    action_number: 1,
                },
            );

            let mut eula_file = PathBuf::new();
            eula_file.push(path);
            eula_file.push(Path::new("eula.txt"));
            fs::write(eula_file, "eula=true\n")
                .map_err(|e| InstallError::WriteInitialSettings(e))?
        }

        self.event_handler.raise_event(
            self.unit_id.as_str(),
            ServerEvent::ActionProgress {
                server_id: self.unit_id.clone(),
                action: "downloading jar".to_string(),
                progress: None,
                maximum: None,
                action_number: 1,
            },
        );

        let server_version = match server_version {
            Some(server_version) => server_version,
            None => PaperRepository::latest_version(),
        };

        let artifact = PaperRepository::get_artifact(server_version);
        let mut dest_path = PathBuf::new();
        dest_path.push(path);

        let jar_name = format!("paper_{}.jar", artifact.version()).replace("+", "-");
        dest_path.push(&jar_name);

        artifact.download_to(dest_path.as_path())?;

        self.event_handler.raise_event(
            self.unit_id.as_str(),
            ServerEvent::ActionProgress {
                server_id: self.unit_id.clone(),
                action: "patching jar".to_string(),
                progress: None,
                maximum: None,
                action_number: 1,
            },
        );

        let mut child = Command::new("java".to_string())
            .arg("-Dpaperclip.patchonly=true")
            .arg("-jar")
            .arg(&jar_name)
            .current_dir(&path)
            .output()
            .unwrap();

        if !child.status.success() {
            Err(InstallError::PerformPatch(
                String::from_utf8_lossy(child.stderr.as_slice()).to_string(),
            ))
        } else {
            Ok(ServerConfig {
                name: server_name.unwrap_or("A Minecraft server".to_string()),
                path: Box::from(path),
                type_name: "paper".to_string(),
                jar: jar_name.to_string(),
                version: artifact.version(),
            })
        }
    }
}
