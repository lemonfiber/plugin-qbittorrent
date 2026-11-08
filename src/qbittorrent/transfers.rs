use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::service::{Download, Failure, TransferState, Transfers};
use serde::Deserialize;

use super::{Qbittorrent, TORRENTS};

const DOWNLOADING: &str = "?filter=downloading";

const ERRORED: &str = "?filter=errored";

const NO_ETA: u64 = 8_640_000;

const WHOLE: u128 = 100;

const DONE: f64 = 1.0;

const STATES: [(&str, TransferState); 9] = [
    ("downloading", TransferState::Downloading),
    ("forcedDL", TransferState::Downloading),
    ("stalledDL", TransferState::Stalled),
    ("metaDL", TransferState::Metadata),
    ("forcedMetaDL", TransferState::Metadata),
    ("queuedDL", TransferState::Queued),
    ("stoppedDL", TransferState::Paused),
    ("error", TransferState::Error),
    ("missingFiles", TransferState::Error),
];

#[derive(Deserialize)]
struct Torrent {
    name: String,
    completed: u64,
    size: u64,
    dlspeed: u64,
    eta: u64,
    hash: Option<String>,
    category: Option<String>,
    state: Option<String>,
    added_on: Option<i64>,
    #[serde(default)]
    progress: f64,
}

#[async_trait]
impl Transfers for Qbittorrent {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let downloading: Vec<Torrent> = self
            .read(
                &format!("{TORRENTS}{DOWNLOADING}"),
                "the torrent list could not be read",
            )
            .await?;
        let errored: Vec<Torrent> = self
            .read(
                &format!("{TORRENTS}{ERRORED}"),
                "the torrents in error could not be read",
            )
            .await?;
        Ok(downloading
            .into_iter()
            .chain(errored.into_iter().filter(|one| one.progress < DONE))
            .map(download_of)
            .collect())
    }
}

fn download_of(torrent: Torrent) -> Download {
    Download {
        name: torrent.name,
        progress: percent(torrent.completed, torrent.size),
        speed: Some(torrent.dlspeed),
        eta: (torrent.eta < NO_ETA).then(|| Duration::from_secs(torrent.eta)),
        remaining: Some(torrent.size.saturating_sub(torrent.completed)),
        id: torrent.hash.filter(|hash| !hash.is_empty()),
        category: torrent.category.filter(|category| !category.is_empty()),
        state: torrent.state.as_deref().and_then(state_of),
        added: torrent
            .added_on
            .and_then(|added| u64::try_from(added).ok())
            .filter(|added| *added > 0),
    }
}

fn state_of(state: &str) -> Option<TransferState> {
    STATES
        .iter()
        .find(|(named, _)| *named == state)
        .map(|(_, said)| *said)
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
