use std::time::Duration;

use lemonfiber_ports::service::{Failure, Transfers};

use super::super::tests::client;
use super::percent;

const TWO_TORRENTS: &str = r#"[
    {"name":"Show.S01E01","completed":500,"size":1000,"dlspeed":2048,"eta":600},
    {"name":"Movie.2024","completed":1000,"size":1000,"dlspeed":0,"eta":8640000}
]"#;

#[tokio::test]
async fn each_torrent_reads_its_progress_speed_and_eta() {
    let (qbittorrent, _) = client(vec![(200, TWO_TORRENTS)]);
    let transfers = qbittorrent.transfers().await.unwrap_or_default();
    assert_eq!(transfers.len(), 2);
    assert!(matches!(
        transfers.first(),
        Some(t) if t.name == "Show.S01E01"
            && t.progress == 50
            && t.speed == Some(2048)
            && t.eta == Some(Duration::from_secs(600))
            && t.remaining == Some(500)
    ));
    assert!(matches!(
        transfers.get(1),
        Some(t) if t.progress == 100 && t.speed == Some(0) && t.eta.is_none()
            && t.remaining == Some(0)
    ));
}

#[tokio::test]
async fn a_torrent_list_that_will_not_parse_is_refused() {
    let (qbittorrent, _) = client(vec![(200, "not json")]);
    assert!(matches!(
        qbittorrent.transfers().await,
        Err(Failure::Refused { .. })
    ));
}

#[test]
fn progress_is_whole_percent_with_nothing_to_do_complete() {
    assert_eq!(percent(1, 3), 33);
    assert_eq!(percent(0, 0), 100);
    assert_eq!(percent(9, 3), 100);
    assert_eq!(percent(u64::MAX, u64::MAX), 100);
}
