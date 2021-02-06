pub mod paper;

use crate::ipc::install::InstallError;
use semver::Version;
use std::path::Path;

pub trait Repository {
    fn get_artifact(version: Version) -> Box<dyn Artifact>;
    fn get_latest_artifact(version: Version) -> Box<dyn Artifact>;

    fn list_versions() -> Vec<Version>;
    fn list_builds(version: Version) -> Vec<Version>;

    fn latest_version() -> Version {
        Self::list_versions().into_iter().max().unwrap()
    }
}

pub trait Artifact {
    fn version(&self) -> Version;

    fn download_to(&self, path: &Path) -> Result<u64, InstallError>;
}
