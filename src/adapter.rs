//! What this adapter says it is, and the kit that serves its contract.

use std::sync::Arc;

use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::download;

use crate::qbittorrent::Qbittorrent;

/// Where qBittorrent answers, by its service id and the port it listens on.
pub const ADDRESS: &str = "http://qbittorrent:8081";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "qBittorrent";

include!(concat!(env!("OUT_DIR"), "/fronted.rs"));

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            download::torrent::CAPABILITY,
            download::torrent::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The contract, served from `qbittorrent` under `key`, ready while qBittorrent answers.
#[must_use]
pub fn kit(key: String, qbittorrent: Arc<Qbittorrent>) -> Kit {
    let asked = Arc::clone(&qbittorrent);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(download::torrent::served(qbittorrent))
}

#[cfg(test)]
mod tests;
