use crate::ipc::install::InstallError;
use crate::repo::{Artifact, Repository};
use semver::{Identifier, Version};
use std::fs::File;
use std::path::Path;

pub struct PaperArtifact {
    version: Version,
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
                "https://papermc.io/api/v1/paper/{}/{}/download",
                reduced_version, build
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
    fn get_artifact(version: Version) -> Box<dyn Artifact> {
        if version.build.is_empty() {
            Self::get_latest_artifact(version)
        } else {
            Box::new(PaperArtifact { version })
        }
    }

    fn get_latest_artifact(version: Version) -> Box<dyn Artifact> {
        let version_string = if version.patch == 0 {
            format!("{}.{}", version.major, version.minor)
        } else {
            version.to_string()
        };
        let response = reqwest::blocking::get(
            format!("https://papermc.io/api/v1/paper/{}/", version_string).as_str(),
        )
        .unwrap();

        let list: BuildList = response.json().unwrap();

        let mut vers = version.clone();
        vers.build.clear();
        vers.build
            .push(Identifier::Numeric(list.builds.latest as u64));
        Box::new(PaperArtifact { version: vers })
    }

    fn list_versions() -> Vec<Version> {
        let response = reqwest::blocking::get("https://papermc.io/api/v1/paper/").unwrap();

        let list: VersionList = response.json().unwrap();

        list.versions
            .iter()
            .map(|version_string| {
                let vers = lenient_semver::parse(version_string.as_str()).unwrap();
                Version::new(vers.major, vers.minor, vers.patch)
            })
            .collect()
    }

    fn list_builds(mut version: Version) -> Vec<Version> {
        let version_string = if version.patch == 0 {
            format!("{}.{}", version.major, version.minor)
        } else {
            version.to_string()
        };
        let response = reqwest::blocking::get(
            format!("https://papermc.io/api/v1/paper/{}/", version_string).as_str(),
        )
        .unwrap();

        let list: BuildList = response.json().unwrap();
        version.build.clear();
        list.builds
            .all
            .iter()
            .map(|build_number| {
                let mut vers = version.clone();
                vers.build.push(Identifier::Numeric(*build_number as u64));
                vers
            })
            .collect()
    }
}

#[derive(Deserialize, Debug)]
struct VersionList {
    project: String,
    versions: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct BuildList {
    project: String,
    version: String,
    builds: Builds,
}

#[derive(Deserialize, Debug)]
struct Builds {
    latest: u32,
    all: Vec<u32>,
}
