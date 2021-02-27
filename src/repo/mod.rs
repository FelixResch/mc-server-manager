//! Basic traits and implementations for dynamic repositories.

pub mod paper;

use crate::ipc::install::InstallError;
use semver::Version;
use std::path::Path;

/// A generic repository for a type of server
pub trait Repository {
    /// Gets the artifact for the given version
    fn get_artifact(version: Version) -> Box<dyn Artifact>;
    /// Gets the latest artifact (build) for the given minecraft version.
    ///
    /// If the server type does not support individual builds, this method MUST return the same
    /// artifact as [get_artifact] does.
    fn get_latest_artifact(version: Version) -> Box<dyn Artifact>;

    /// List the available versions (without builds) for a type of server.
    fn list_versions() -> Vec<Version>;
    /// List all available builds for a given version. The build information MUST be included in
    /// the returned versions and `MAJOR.MINOR.PATCH` of all returned versions MUST equal the
    /// respective components in `version`.
    fn list_builds(version: Version) -> Vec<Version>;

    /// Returns the latest version for a server type.
    fn latest_version() -> Version {
        Self::list_versions().into_iter().max().unwrap()
    }
}

pub trait Artifact {
    fn version(&self) -> Version;

    fn download_to(&self, path: &Path) -> Result<u64, InstallError>;
}
