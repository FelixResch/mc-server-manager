//! Traits and implementations for server updaters

use crate::config::ServerConfig;
use crate::daemon::event::EventHandler;
use crate::ipc::install::InstallError;
use crate::ipc::ServerEvent;
use crate::repo::paper::PaperRepository;
use crate::repo::Repository;
use crate::ServerType;
use semver::Version;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Command;

/// Trait for server updaters.
pub trait ServerUpdater {
    /// Updates an existing server to the given version. If no version is provided the server
    /// should be updated to the latest version.
    // This behaviour should be discussed further: Maybe it would be better to only to minor version
    // updates automatically? What about updates for build versions?
    fn update_server(
        &mut self,
        server_version: Option<Version>,
        server_config: ServerConfig,
    ) -> Result<ServerConfig, UpdateError>;
}

/// Errors that can happen during an update operations
#[derive(Debug)]
pub enum UpdateError {
    /// Download of an artifact or an additional resource has failed
    DownloadFailed(Box<dyn Error + 'static + Send>),
    /// The patching process (Vanilla -> other server) failed.
    ///
    /// The passed String contains additional info or error messages.
    ///
    /// This error MUST only be sent by installers that patch the Vanilla jar.
    PerformPatch(String),
    /// An error occurred during the writing of the unit file.
    ///
    /// This usually indicates wrong file permissions.
    WriteUnitFile(io::Error),
    /// The server is already using the current version/build
    AlreadyUpToDate,
    /// The updater does not support updating the requested server port
    UnsupportedServerType(ServerType),
}

/// Updater for paper servers
pub struct PaperServerUpdater {
    /// The id of the unit, that should be updated
    unit_id: String,
    /// The path to the file of the unit
    #[allow(dead_code)]
    unit_file_path: PathBuf,
    /// an event handler which is connected to the main event manager
    event_handler: EventHandler,
}

impl PaperServerUpdater {
    /// Create a new updater for paper servers
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

        if current_version >= target_artifact.version()
            && current_version
                .build
                .first()
                .expect("version build information")
                >= target_artifact
                    .version()
                    .build
                    .first()
                    .expect("version build information")
        {
            return Err(UpdateError::AlreadyUpToDate);
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

        let child = Command::new("java".to_string())
            .arg("-Dpaperclip.patchonly=true")
            .arg("-jar")
            .arg(&jar_name)
            .current_dir(&path)
            .output()
            .expect("start patch command");

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
