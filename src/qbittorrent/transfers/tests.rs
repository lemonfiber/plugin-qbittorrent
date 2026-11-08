use std::time::Duration;

use lemonfiber_ports::service::{Failure, TransferState, Transfers};

use super::super::tests::{asked, client, BASE};
use super::{percent, state_of};

const TWO_TORRENTS: &str = r#"[
    {"name":"Show.S01E01","completed":500,"size":1000,"dlspeed":2048,"eta":600,
     "hash":"0123456789abcdef0123456789abcdef01234567","category":"tv","state":"downloading",
     "added_on":1794156709,"progress":0.5},
    {"name":"Movie.2024","completed":1000,"size":1000,"dlspeed":0,"eta":8640000,
     "hash":"89abcdef0123456789abcdef0123456789abcdef","category":"","state":"checkingDL",
     "added_on":-1,"progress":1}
]"#;

const ERRORED: &str = r#"[
    {"name":"Broken.S02E01","completed":10,"size":1000,"dlspeed":0,"eta":8640000,
     "hash":"fedcba9876543210fedcba9876543210fedcba98","category":"tv","state":"error",
     "added_on":1794150000,"progress":0.01},
    {"name":"Seeded.2023","completed":1000,"size":1000,"dlspeed":0,"eta":8640000,
     "hash":"76543210fedcba9876543210fedcba9876543210","category":"movies","state":"missingFiles",
     "added_on":1794140000,"progress":1}
]"#;

const BARE: &str = r#"[{"name":"Old","completed":0,"size":0,"dlspeed":0,"eta":0,"added_on":0}]"#;

#[tokio::test]
async fn each_torrent_reads_its_progress_speed_and_eta() {
    let (qbittorrent, _) = client(vec![(200, TWO_TORRENTS), (200, "[]")]);
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
async fn each_torrent_names_its_hash_category_state_and_when_it_was_added() {
    let (qbittorrent, _) = client(vec![(200, TWO_TORRENTS), (200, "[]")]);
    let transfers = qbittorrent.transfers().await.unwrap_or_default();
    assert!(matches!(
        transfers.first(),
        Some(t) if t.id.as_deref() == Some("0123456789abcdef0123456789abcdef01234567")
            && t.category.as_deref() == Some("tv")
            && t.state == Some(TransferState::Downloading)
            && t.added == Some(1_794_156_709)
    ));
    assert!(matches!(
        transfers.get(1),
        Some(t) if t.id.as_deref() == Some("89abcdef0123456789abcdef0123456789abcdef")
            && t.category.is_none()
            && t.state.is_none()
            && t.added.is_none()
    ));
}

#[tokio::test]
async fn a_torrent_in_error_is_a_transfer_until_it_has_everything() {
    let (qbittorrent, http) = client(vec![(200, "[]"), (200, ERRORED)]);
    let transfers = qbittorrent.transfers().await.unwrap_or_default();
    assert_eq!(transfers.len(), 1);
    assert!(matches!(
        transfers.first(),
        Some(t) if t.name == "Broken.S02E01"
            && t.state == Some(TransferState::Error)
            && t.added == Some(1_794_150_000)
    ));
    assert_eq!(
        asked(&http)
            .into_iter()
            .map(|(_, url, _)| url)
            .collect::<Vec<_>>(),
        [
            format!("{BASE}/api/v2/torrents/info?filter=downloading"),
            format!("{BASE}/api/v2/torrents/info?filter=errored"),
        ]
    );
}

#[tokio::test]
async fn a_torrent_naming_none_of_the_new_fields_answers_none_of_them() {
    let (qbittorrent, _) = client(vec![(200, BARE), (200, "[]")]);
    let transfers = qbittorrent.transfers().await.unwrap_or_default();
    assert!(matches!(
        transfers.first(),
        Some(t) if t.id.is_none() && t.category.is_none() && t.state.is_none()
            && t.added.is_none()
    ));
}

#[tokio::test]
async fn a_torrent_list_that_will_not_parse_is_refused() {
    let (qbittorrent, _) = client(vec![(200, "not json")]);
    assert!(matches!(
        qbittorrent.transfers().await,
        Err(Failure::Refused { .. })
    ));
    let (errored, _) = client(vec![(200, "[]"), (200, "not json")]);
    assert!(matches!(
        errored.transfers().await,
        Err(Failure::Refused { .. })
    ));
}

#[test]
fn every_state_of_an_unfinished_torrent_is_said_in_the_contract_s_words() {
    for (named, said) in [
        ("downloading", TransferState::Downloading),
        ("forcedDL", TransferState::Downloading),
        ("stalledDL", TransferState::Stalled),
        ("metaDL", TransferState::Metadata),
        ("forcedMetaDL", TransferState::Metadata),
        ("queuedDL", TransferState::Queued),
        ("stoppedDL", TransferState::Paused),
        ("error", TransferState::Error),
        ("missingFiles", TransferState::Error),
    ] {
        assert_eq!(state_of(named), Some(said), "{named}");
    }
    for unsaid in ["checkingDL", "checkingResumeData", "moving", "unknown", ""] {
        assert_eq!(state_of(unsaid), None, "{unsaid}");
    }
}

#[test]
fn progress_is_whole_percent_with_nothing_to_do_complete() {
    assert_eq!(percent(1, 3), 33);
    assert_eq!(percent(0, 0), 100);
    assert_eq!(percent(9, 3), 100);
    assert_eq!(percent(u64::MAX, u64::MAX), 100);
}
