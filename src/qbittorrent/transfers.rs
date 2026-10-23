use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::service::{Download, Failure, Transfers};
use serde::Deserialize;

use super::{Qbittorrent, TORRENTS};

const DOWNLOADING: &str = "?filter=downloading";

const NO_ETA: u64 = 8_640_000;

const WHOLE: u128 = 100;

#[derive(Deserialize)]
struct Torrent {
    name: String,
    completed: u64,
    size: u64,
    dlspeed: u64,
    eta: u64,
}

#[async_trait]
impl Transfers for Qbittorrent {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let torrents: Vec<Torrent> = self
            .read(
                &format!("{TORRENTS}{DOWNLOADING}"),
                "the torrent list could not be read",
            )
            .await?;
        Ok(torrents.into_iter().map(download_of).collect())
    }
}

fn download_of(torrent: Torrent) -> Download {
    Download {
        name: torrent.name,
        progress: percent(torrent.completed, torrent.size),
        speed: Some(torrent.dlspeed),
        eta: (torrent.eta < NO_ETA).then(|| Duration::from_secs(torrent.eta)),
        remaining: Some(torrent.size.saturating_sub(torrent.completed)),
    }
}

fn percent(done: u64, total: u64) -> u8 {
    let whole = u128::from(done)
        .saturating_mul(WHOLE)
        .checked_div(u128::from(total))
        .map_or(WHOLE, |ratio| ratio.min(WHOLE));
    u8::try_from(whole).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests;
