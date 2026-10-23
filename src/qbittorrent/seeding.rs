use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Seeded, Seeding};
use serde::Deserialize;

use super::{Qbittorrent, TORRENTS};

const COMPLETED: &str = "?filter=completed";

const DELETE: &str = "/torrents/delete";

const HUNDRED: u64 = 100;

#[derive(Deserialize)]
struct Completed {
    hash: String,
    name: String,
    size: u64,
    uploaded: u64,
    downloaded: u64,
}

impl Qbittorrent {
    async fn completed(&self) -> Result<Vec<Completed>, Failure> {
        self.read(
            &format!("{TORRENTS}{COMPLETED}"),
            "the completed torrent list could not be read",
        )
        .await
    }
}

#[async_trait]
impl Seeding for Qbittorrent {
    async fn seeding(&self) -> Result<Vec<Seeded>, Failure> {
        Ok(self.completed().await?.into_iter().map(seeded_of).collect())
    }

    async fn stop_seeding(&self, name: &str) -> Result<(), Failure> {
        let holding = self.completed().await?;
        let named: Vec<&Completed> = holding.iter().filter(|one| one.name == name).collect();
        let [one] = named.as_slice() else {
            return Err(self.endpoint.refused(&format!(
                "the client is holding {} completed downloads called {name}",
                named.len()
            )));
        };
        self.wrote(
            DELETE,
            &[("hashes", one.hash.as_str()), ("deleteFiles", "true")],
        )
        .await?;
        if self.completed().await?.iter().any(|one| one.name == name) {
            return Err(self
                .endpoint
                .refused(&format!("the client is still holding {name}")));
        }
        Ok(())
    }
}

fn seeded_of(torrent: Completed) -> Seeded {
    Seeded {
        name: torrent.name,
        bytes: torrent.size,
        ratio: hundredths(torrent.uploaded, torrent.downloaded),
    }
}

fn hundredths(uploaded: u64, downloaded: u64) -> u32 {
    uploaded
        .saturating_mul(HUNDRED)
        .checked_div(downloaded)
        .map_or(u32::MAX, |scaled| u32::try_from(scaled).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests;
