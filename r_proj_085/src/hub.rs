//! Minimal blocking Hugging Face Hub client, adapted from candle-examples/src/hub.rs
//! for hf-hub 1.0's `HFClientSync` API.

use hf_hub::{HFClientSync, HFError, HFRepositorySync, HFResult, RepoType, RepoTypeModel};
use std::path::PathBuf;

pub struct Api(HFClientSync);

impl Api {
    pub fn new() -> HFResult<Self> {
        Ok(Self(HFClientSync::new()?))
    }

    pub fn model(&self, id: impl AsRef<str>) -> Repo<RepoTypeModel> {
        let (owner, name) = hf_hub::split_id(id.as_ref());
        Repo::new(self.0.model(owner, name))
    }
}

pub struct Repo<T: RepoType> {
    repo: HFRepositorySync<T>,
    revision: Option<String>,
}

impl<T: RepoType> Repo<T> {
    pub fn new(repo: HFRepositorySync<T>) -> Self {
        Self {
            repo,
            revision: None,
        }
    }

    pub fn with_revision<S: AsRef<str>>(mut self, revision: S) -> Self {
        self.revision = Some(revision.as_ref().to_string());
        self
    }

    pub fn get<S: AsRef<str>>(&self, filename: S) -> HFResult<PathBuf> {
        let filename = filename.as_ref();
        let download = self
            .repo
            .download_file()
            .filename(filename)
            .maybe_revision(self.revision.clone());
        match download.clone().local_files_only(true).send() {
            Err(HFError::LocalEntryNotFound { .. }) => download.send(),
            cached => cached,
        }
    }
}