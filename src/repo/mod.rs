//! Basic traits and implementations for dynamic repositories.

pub mod paper;

use std::borrow::Cow;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use crate::ipc::install::InstallError;
use semver::Version;
use std::path::Path;

#[derive(Debug)]
pub struct RepositoryError {
    inner: Option<Box<dyn Error + Send>>,
    message: Cow<'static, str>,
}

impl RepositoryError {
    pub const fn borrowed(message: &'static str) -> Self {
        Self {
            inner: None,
            message: Cow::Borrowed(message),
        }
    }

    pub fn owned(message: String) -> Self {
        Self {
            inner: None,
            message: Cow::Owned(message),
        }
    }

    pub fn with_inner(self, inner: Box<dyn Error + Send>) -> Self {
        Self {
            inner: Some(inner),
            message: self.message,
        }
    }
}

impl Display for RepositoryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(inner) = self.inner.as_ref() {
            write!(f, "{}: {}", self.message, inner)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl Error for RepositoryError {

}

type RepositoryResult<T> = Result<T, RepositoryError>;

/// A generic repository for a type of server
pub trait Repository {
    /// Gets the artifact for the given version
    fn get_artifact(&self, version: Version) -> RepositoryResult<Box<dyn Artifact>>;
    /// Gets the latest artifact (build) for the given minecraft version.
    ///
    /// If the server type does not support individual builds, this method MUST return the same
    /// artifact as [get_artifact] does.
    fn get_latest_artifact(&self, version: Version) -> RepositoryResult<Box<dyn Artifact>>;

    /// List the available versions (without builds) for a type of server.
    fn list_versions(&self) -> RepositoryResult<Vec<Version>>;
    /// List all available builds for a given version. The build information MUST be included in
    /// the returned versions and `MAJOR.MINOR.PATCH` of all returned versions MUST equal the
    /// respective components in `version`.
    fn list_builds(&self, version: Version) -> RepositoryResult<Vec<Version>>;

    /// Returns the latest version for a server type.
    fn latest_version(&self) -> RepositoryResult<Version> {
        self.list_versions()?.into_iter().max()
            .ok_or(RepositoryError::borrowed("no version found"))
    }
}

pub trait Artifact: Debug {
    fn version(&self) -> Version;

    fn download_to(&self, path: &Path) -> Result<u64, InstallError>;
}
