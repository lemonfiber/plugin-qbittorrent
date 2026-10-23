//! lemonfiber's adapter for qBittorrent: the `download.torrent` contract, spoken in front
//! of one qBittorrent and translated to its own web UI API.

pub mod adapter;
pub mod credential;
mod endpoint;
pub mod qbittorrent;
pub mod settings;
pub mod web;
