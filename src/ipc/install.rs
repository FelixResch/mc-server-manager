//! Contains basic traits and some implemetations for installing servers.

mod iops;

use crate::config::ServerConfig;
use crate::daemon::event::EventHandler;
use crate::ipc::ServerEvent;
use crate::repo::paper::PaperRepository;
use crate::repo::Repository;
use crate::ServerType;
use semver::Version;
use std::error::Error;
use std::fs::{self, create_dir_all};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Supertype for server installers. All installers must implement this trait and must be compatible
/// with this interface.
pub trait ServerInstaller {
    /// Installs a server to the given `install_path`.
    ///
    /// If no `server_version` is provided the implementation MUST choose the latest available
    /// build and version. Server types that do not use the build parameter can ignore the build
    /// parameter.
    ///
    /// Implementations MUST NOT accept the EULA if `eula` is not set. That way users have to
    /// actively accept the EULA.
    ///
    /// The `server_name` SHOULD be used as the name that is displayed by the server.
    fn install_server(
        &mut self,
        install_path: String,
        server_version: Option<Version>,
        eula: bool,
        server_name: Option<String>,
    ) -> Result<ServerConfig, InstallError>;
}

#[derive(Debug)]
/// Errors that can happen during installation of a server.
pub enum InstallError {
    /// The installation directory exists.
    DirExists,
    /// The installer was unable to create the installation directory
    CreateDir(io::Error),
    /// The download of the server artifact or other artifacts failed.
    DownloadFailed(Box<dyn Error + 'static + Send>),
    /// The initial settings could not be written. (e.g. eula, server name, ...)
    WriteInitialSettings(io::Error),
    /// The patching process (Vanilla -> other server) failed.
    ///
    /// The passed String contains additional info or error messages.
    ///
    /// This error MUST only be sent by installers that patch the Vanilla jar.
    PerformPatch(String),
    /// The unit file could not be written.
    ///
    /// This usually indicates wrong file permission for the daemon or wrong systemd configuration.
    WriteUnitFile(io::Error),
    /// The used server type is not supported.
    ///
    /// Some installers might support multiple server types (e.g. Spigot).
    UnsupportedServerType(ServerType),
    /// The unit already exists.
    ///
    /// This error type should only be used by the daemon, not by an installer.
    UnitAlreadyExists,
}

/// Installer implementation for PaperMC
pub struct PaperServerInstaller {
    /// An event handler with a connection to the main event manager
    event_handler: EventHandler,
    /// The id of the unit to be installed
    unit_id: String,
    repo: PaperRepository,
}

impl PaperServerInstaller {
    /// Create a new PaperServerInstaller
    pub fn new(event_handler: EventHandler, unit_id: String) -> PaperServerInstaller {
        Self {
            event_handler,
            unit_id,
            repo: PaperRepository {},
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
            return Err(InstallError::DirExists);
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

        create_dir_all(path).map_err(InstallError::CreateDir)?;

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
            fs::write(eula_file, "eula=true\n").map_err(InstallError::WriteInitialSettings)?
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
            None => self.repo.latest_version().map_err(|e| InstallError::DownloadFailed(Box::new(e)))?,
        };

        let artifact = self.repo.get_artifact(server_version).map_err(|e| InstallError::DownloadFailed(Box::new(e)))?;
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

        let child = Command::new("java".to_string())
            .arg("-Dpaperclip.patchonly=true")
            .arg("-jar")
            .arg(&jar_name)
            .current_dir(&path)
            .output()
            .expect("spawn path process");

        if !child.status.success() {
            Err(InstallError::PerformPatch(
                String::from_utf8_lossy(child.stderr.as_slice()).to_string(),
            ))
        } else {
            Ok(ServerConfig {
                name: server_name.unwrap_or_else(|| "A Minecraft server".to_string()),
                path: Box::from(path),
                type_name: "paper".to_string(),
                jar: jar_name,
                version: artifact.version(),
                memory: 10,
            })
        }
    }
}
