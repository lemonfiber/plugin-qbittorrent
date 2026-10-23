use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Fetching, Pulling};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Qbittorrent, PREFERENCES, TORRENTS};

const RUNNING: &str = "?filter=running";

const EVERY: &str = "all";

const ADD_STOPPED: &str = "add_stopped_enabled";

#[derive(Deserialize)]
struct Adding {
    #[serde(default)]
    add_stopped_enabled: bool,
}

#[derive(Deserialize)]
struct Running {}

impl Qbittorrent {
    async fn adding_stopped(&self, stopped: bool) -> Result<(), Failure> {
        self.prefer(Map::from_iter([(
            ADD_STOPPED.to_owned(),
            Value::Bool(stopped),
        )]))
        .await
    }

    async fn every_torrent(&self, action: &str) -> Result<(), Failure> {
        self.wrote(&format!("/torrents/{action}"), &[("hashes", EVERY)])
            .await
    }
}

#[async_trait]
impl Fetching for Qbittorrent {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        let running: Vec<Running> = self
            .read(
                &format!("{TORRENTS}{RUNNING}"),
                "what the client is running could not be read",
            )
            .await?;
        let adding: Adding = self
            .read(PREFERENCES, "how new downloads are added could not be read")
            .await?;
        Ok(Pulling::of(
            !running.is_empty(),
            !adding.add_stopped_enabled,
        ))
    }

    async fn stop(&self) -> Result<Pulling, Failure> {
        self.adding_stopped(true).await?;
        self.every_torrent("stop").await?;
        self.pulling().await
    }

    async fn resume(&self) -> Result<Pulling, Failure> {
        self.adding_stopped(false).await?;
        self.every_torrent("start").await?;
        self.pulling().await
    }
}

#[cfg(test)]
mod tests;
