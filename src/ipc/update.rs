use crate::config::{ServerConfig, UnitConfig};
use crate::daemon::event::EventHandler;
use crate::ipc::install::InstallError;
use crate::ipc::ServerEvent;
use crate::repo::paper::PaperRepository;
use crate::repo::Repository;
use crate::ServerType;
use semver::Version;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub trait ServerUpdater {
    fn update_server(
        &mut self,
        server_version: Option<Version>,
        server_config: ServerConfig,
    ) -> Result<ServerConfig, UpdateError>;
}

#[derive(Debug)]
pub enum UpdateError {
    DownloadFailed(Box<dyn Error + 'static + Send>),
    PerformPatch(String),
    WriteUnitFile(io::Error),
    AlreadyUpToDate,
    UnsupportedServerType(ServerType),
}

pub struct PaperServerUpdater {
    unit_id: String,
    unit_file_path: PathBuf,
    event_handler: EventHandler,
}

impl PaperServerUpdater {
    pub fn new(unit_id: String, unit_file_path: PathBuf, event_handler: EventHandler) -> Self {
        Self {
            unit_id,
            unit_file_path,
            event_handler,
        }
    }
}

impl ServerUpdater for PaperServerUpdater {
    fn update_server(
        &mut self,
        server_version: Option<Version>,
        mut server_config: ServerConfig,
    ) -> Result<ServerConfig, UpdateError> {
        let path = server_config.path.as_ref();
        self.event_handler.raise_event(
            self.unit_id.as_str(),
            ServerEvent::ActionProgress {
                server_id: self.unit_id.clone(),
                action: "checking for update".to_string(),
                progress: None,
                maximum: None,
                action_number: 1,
            },
        );

        let current_version = server_config.version.clone();
        let target_artifact = match server_version {
            None => {
                let latest_version = PaperRepository::latest_version();
                PaperRepository::get_latest_artifact(latest_version)
            }
            Some(target_version) => PaperRepository::get_artifact(target_version),
        };

        if current_version >= target_artifact.version() {
            if current_version.build.first().unwrap()
                >= target_artifact.version().build.first().unwrap()
            {
                return Err(UpdateError::AlreadyUpToDate);
            }
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

        let mut dest_path = PathBuf::new();
        dest_path.push(&path);

        let jar_name = format!("paper_{}.jar", target_artifact.version()).replace("+", "-");
        dest_path.push(&jar_name);

        target_artifact
            .download_to(dest_path.as_path())
            .map_err(|e| match e {
                InstallError::DownloadFailed(e) => UpdateError::DownloadFailed(e),
                _ => unimplemented!(),
            })?;

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
            Err(UpdateError::PerformPatch(
                String::from_utf8_lossy(child.stderr.as_slice()).to_string(),
            ))
        } else {
            server_config.version = target_artifact.version();
            server_config.jar = jar_name;
            Ok(server_config)
        }
    }
}
