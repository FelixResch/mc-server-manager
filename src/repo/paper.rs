use std::collections::HashMap;
use crate::ipc::install::InstallError;
use crate::repo::{Artifact, Repository, RepositoryError, RepositoryResult};
use semver::{Identifier, Version};
use std::fs::File;
use std::path::Path;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct PaperArtifact {
    version: Version,
    download: Download,
}

impl Artifact for PaperArtifact {
    fn version(&self) -> Version {
        self.version.clone()
    }

    fn download_to(&self, path: &Path) -> Result<u64, InstallError> {
        let reduced_version =
            Version::new(self.version.major, self.version.minor, self.version.patch);
        let build = self.version.build.first().unwrap().to_string();

        let response = reqwest::blocking::get(
            format!(
                "https://papermc.io/api/v2/projects/paper/versions/{}/builds/{}/downloads/{}",
                reduced_version, build, self.download.name
            )
            .as_str(),
        );

        match response {
            Ok(mut response) => {
                let mut file = File::create(&path).unwrap();
                Ok(response
                    .copy_to(&mut file)
                    .map_err(|e| InstallError::DownloadFailed(Box::new(e)))?)
            }
            Err(e) => Err(InstallError::DownloadFailed(Box::new(e))),
        }
    }
}

pub struct PaperRepository {}

impl Repository for PaperRepository {
    fn get_artifact(&self, version: Version) -> RepositoryResult<Box<dyn Artifact>> {
        if version.build.is_empty() {
            self.get_latest_artifact(version)
        } else {
            let build = version.build.first().expect("other case is handled above").to_string();
            let version_string = if version.patch == 0 {
                format!("{}.{}", version.major, version.minor)
            } else {
                version.to_string()
            };

            let response = reqwest::blocking::get(
                &format!("https://papermc.io/api/v2/projects/paper/versions/{}/builds/{}", version_string, build)
            )
                .map_err(|e| RepositoryError::borrowed("failed to execute repository request").with_inner(Box::new(e)))?;

            let mut build_response: BuildResponse = response.json().map_err(|e| RepositoryError::borrowed("failed to parse json response").with_inner(Box::new(e)))?;

            let download = build_response.downloads.remove("application").ok_or(RepositoryError::borrowed("no application download present"))?;
            Ok(
                Box::new(PaperArtifact { version, download })
            )
        }
    }

    fn get_latest_artifact(&self, version: Version) -> RepositoryResult<Box<dyn Artifact>> {
        let version_string = if version.patch == 0 {
            format!("{}.{}", version.major, version.minor)
        } else {
            version.to_string()
        };
        let response = reqwest::blocking::get(
            format!("https://papermc.io/api/v2/projects/paper/versions/{}/builds", version_string).as_str(),
        )
            .map_err(|e| RepositoryError::borrowed("failed to execute repository request").with_inner(Box::new(e)))?;

        let build_list: BuildList = response.json().map_err(|e| RepositoryError::borrowed("failed to parse json response").with_inner(Box::new(e)))?;

        let mut last_build = build_list.builds.into_iter().max_by_key(|build| build.build)
            .ok_or(RepositoryError::borrowed("no build found for the given version"))?;

        let mut vers = version.clone();
        vers.build.clear();
        vers.build
            .push(Identifier::Numeric(last_build.build));

        let download = last_build.downloads.remove("application").ok_or(RepositoryError::borrowed("no application download present"))?;
        Ok(
            Box::new(PaperArtifact { version: vers, download })
        )
    }

    fn list_versions(&self) -> RepositoryResult<Vec<Version>> {
        let response = reqwest::blocking::get("https://papermc.io/api/v2/projects/paper/").map_err(|e| RepositoryError::borrowed("failed to execute repository request").with_inner(Box::new(e)))?;

        let list: ProjectResponse = response.json().map_err(|e| RepositoryError::borrowed("failed to parse json response").with_inner(Box::new(e)))?;

        list.versions
            .iter()
            .map(|version_string| {
                let vers = lenient_semver::parse(version_string.as_str()).map_err(|e| RepositoryError::borrowed("failed to parse version string"))?;
                Ok(
                    Version::new(vers.major, vers.minor, vers.patch)
                )
            })
            .collect()
    }

    fn list_builds(&self, mut version: Version) -> RepositoryResult<Vec<Version>> {
        let version_string = if version.patch == 0 {
            format!("{}.{}", version.major, version.minor)
        } else {
            version.to_string()
        };
        let response = reqwest::blocking::get(
            format!("https://papermc.io/api/v2/projects/paper/versions/{}/builds", version_string).as_str(),
        )
            .map_err(|e| RepositoryError::borrowed("failed to execute repository request").with_inner(Box::new(e)))?;

        let list: BuildList = response.json().map_err(|e| RepositoryError::borrowed("failed to parse json response").with_inner(Box::new(e)))?;
        version.build.clear();
        Ok(
            list.builds
            .iter()
            .map(|build_number| {
                let mut vers = version.clone();
                vers.build.push(Identifier::Numeric(build_number.build));
                vers
            })
            .collect()
        )
    }
}

#[derive(Deserialize, Debug)]
struct ProjectResponse {
    project_id: String,
    project_name: String,
    version_groups: Vec<String>,
    versions: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct BuildList {
    project_id: String,
    project_name: String,
    version: String,
    builds: Vec<VersionBuild>,
}

#[derive(Deserialize, Debug)]
struct VersionBuild {
    build: u64,
    time: DateTime<Utc>,
    channel: BuildChannel,
    promoted: bool,
    changes: Vec<Change>,
    downloads: HashMap<String, Download>,
}

#[derive(Deserialize, Debug)]
enum BuildChannel {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "experimental")]
    Experimental,
}

#[derive(Deserialize, Debug)]
struct Change {
    commit: String,
    summary: String,
    message: String,
}

#[derive(Deserialize, Debug)]
struct Download {
    name: String,
    sha256: String,
}

#[derive(Deserialize, Debug)]
struct BuildResponse {
    project_id: String,
    project_name: String,
    version: String,
    build: u64,
    channel: BuildChannel,
    promoted: bool,
    changes: Vec<Change>,
    downloads: HashMap<String, Download>,
}

#[cfg(test)]
mod tests {
    use crate::repo::paper::PaperRepository;
    use crate::repo::Repository;

    #[test]
    fn test_versions() {
        let repo = PaperRepository {};
        let versions = repo.list_versions();
        println!("versions: {:?}", versions);
    }

    #[test]
    fn test_latest_artifact() {
        let repo = PaperRepository {};
        let latest_version = repo.latest_version();
        assert!(latest_version.is_ok());
        let latest_version = latest_version.unwrap();

        let latest_artifact = repo.get_latest_artifact(latest_version).unwrap();

        println!("latest artifact: {:?}", latest_artifact)
    }
}