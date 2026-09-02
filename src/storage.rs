use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::{fs, sync::RwLock};

use crate::model::GuildConfig;

#[derive(Debug, Clone)]
pub struct Storage {
    inner: Arc<StorageInner>,
}

#[derive(Debug)]
struct StorageInner {
    directory: PathBuf,
    cache: RwLock<HashMap<u64, GuildConfig>>,
}

impl Storage {
    pub async fn open(directory: impl AsRef<Path>) -> Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)
            .await
            .with_context(|| format!("failed to create {}", directory.display()))?;

        Ok(Self {
            inner: Arc::new(StorageInner {
                directory,
                cache: RwLock::new(HashMap::new()),
            }),
        })
    }

    pub async fn guild_config(&self, guild_id: u64) -> Result<GuildConfig> {
        if let Some(config) = self.inner.cache.read().await.get(&guild_id).cloned() {
            return Ok(config);
        }

        let path = self.path_for(guild_id);
        let config = match fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid guild configuration in {}", path.display()))?,
            Err(error) if error.kind() == ErrorKind::NotFound => GuildConfig::default(),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", path.display()));
            }
        };

        self.inner
            .cache
            .write()
            .await
            .insert(guild_id, config.clone());
        Ok(config)
    }

    pub async fn save_guild_config(&self, guild_id: u64, config: GuildConfig) -> Result<()> {
        let path = self.path_for(guild_id);
        let temporary = self.inner.directory.join(format!(".{guild_id}.json.tmp"));
        let body = serde_json::to_vec_pretty(&config)?;

        fs::write(&temporary, body)
            .await
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        fs::rename(&temporary, &path)
            .await
            .with_context(|| format!("failed to replace {}", path.display()))?;

        self.inner.cache.write().await.insert(guild_id, config);
        Ok(())
    }

    pub async fn module_enabled(&self, guild_id: u64, module: &str) -> Result<bool> {
        Ok(self.guild_config(guild_id).await?.module_enabled(module))
    }

    pub async fn module_config(&self, guild_id: u64, module: &str) -> Result<Value> {
        Ok(self.guild_config(guild_id).await?.config_for(module))
    }

    fn path_for(&self, guild_id: u64) -> PathBuf {
        self.inner.directory.join(format!("{guild_id}.json"))
    }
}
