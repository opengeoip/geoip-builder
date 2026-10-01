use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use ureq::Agent;

#[derive(Clone, Debug)]
pub struct Source {
    pub name: String,
    pub url: String,
}

impl Source {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
        }
    }

    pub fn path(&self, dir: &Path) -> PathBuf {
        dir.join(&self.name)
    }

    fn meta_path(&self, dir: &Path) -> PathBuf {
        dir.join(format!("{}.meta.json", self.name))
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Meta {
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_at: u64,
    pub size: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Downloaded(u64),
    NotModified,
}

pub fn is_rate_limited(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ureq::Error>(),
        Some(ureq::Error::StatusCode(429))
    )
}

#[derive(Clone, Debug)]
pub struct Options {
    pub connect_timeout: Duration,
    pub global_timeout: Option<Duration>,
    pub max_size: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(30),
            global_timeout: None,
            max_size: u64::MAX,
        }
    }
}

pub struct Fetcher {
    agent: Agent,
    dir: PathBuf,
    max_size: u64,
}

impl Fetcher {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_options(dir, Options::default())
    }

    pub fn with_options(dir: impl Into<PathBuf>, options: Options) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let agent = Agent::config_builder()
            .user_agent(concat!("geoip-builder/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(options.connect_timeout))
            .timeout_global(options.global_timeout)
            .build()
            .new_agent();
        Ok(Self {
            agent,
            dir,
            max_size: options.max_size,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn fetch(&self, source: &Source) -> Result<Outcome> {
        let path = source.path(&self.dir);
        let meta_path = source.meta_path(&self.dir);
        let previous: Option<Meta> = path
            .exists()
            .then(|| fs::read(&meta_path).ok())
            .flatten()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .filter(|meta: &Meta| meta.url == source.url);

        let mut request = self.agent.get(&source.url);
        if let Some(meta) = &previous {
            if let Some(etag) = &meta.etag {
                request = request.header("If-None-Match", etag);
            }
            if let Some(last_modified) = &meta.last_modified {
                request = request.header("If-Modified-Since", last_modified);
            }
        }
        let response = request
            .call()
            .with_context(|| format!("GET {}", source.url))?;
        if response.status() == 304 {
            return Ok(Outcome::NotModified);
        }
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let etag = header("etag");
        let last_modified = header("last-modified");

        let partial = self.dir.join(format!("{}.part", source.name));
        let mut reader = response
            .into_body()
            .into_with_config()
            .limit(self.max_size)
            .reader();
        let mut writer = BufWriter::new(File::create(&partial)?);
        let size = match io::copy(&mut reader, &mut writer) {
            Ok(size) => size,
            Err(error) => {
                drop(writer);
                let _ = fs::remove_file(&partial);
                return Err(error).with_context(|| format!("downloading {}", source.url));
            }
        };
        writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
        fs::rename(&partial, &path)?;

        let meta = Meta {
            url: source.url.clone(),
            etag,
            last_modified,
            fetched_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            size,
        };
        let mut file = File::create(&meta_path)?;
        file.write_all(&serde_json::to_vec_pretty(&meta)?)?;
        Ok(Outcome::Downloaded(size))
    }
}
