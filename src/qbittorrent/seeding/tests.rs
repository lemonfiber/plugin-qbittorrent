use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Seeding};

use super::super::tests::{asked, client, BASE};
use super::hundredths;

const THREE_COMPLETED: &str = r#"[
    {"hash":"aa","name":"Show.S01E01","size":1000,"uploaded":1750,"downloaded":1000},
    {"hash":"bb","name":"Movie.2024","size":4000,"uploaded":7,"downloaded":4000},
    {"hash":"cc","name":"Already.Here","size":9000,"uploaded":100,"downloaded":0}
]"#;

const ONE_COMPLETED: &str =
    r#"[{"hash":"a1","name":"Show.S01E01","size":1000,"uploaded":1750,"downloaded":1000}]"#;

const TWO_OF_A_NAME: &str = r#"[
    {"hash":"a1","name":"Show.S01E01","size":1000,"uploaded":1750,"downloaded":1000},
    {"hash":"b2","name":"Show.S01E01","size":1000,"uploaded":10,"downloaded":1000}
]"#;

const NONE: (u16, &str) = (200, "[]");

const ACCEPTED: (u16, &str) = (200, "");

#[tokio::test]
async fn each_completed_torrent_reads_what_it_holds_and_what_it_has_given_back() {
    let (qbittorrent, _) = client(vec![(200, THREE_COMPLETED)]);
    let held = qbittorrent.seeding().await.unwrap_or_default();
    assert_eq!(held.len(), 3);
    assert!(matches!(
        held.first(),
        Some(one) if one.name == "Show.S01E01" && one.bytes == 1000 && one.ratio == 175
    ));
    assert!(matches!(held.get(1), Some(one) if one.ratio == 0));
    assert!(matches!(held.get(2), Some(one) if one.ratio == u32::MAX));
}

#[tokio::test]
async fn a_completed_list_that_will_not_read_is_a_failure_rather_than_none() {
    let (qbittorrent, _) = client(vec![(200, "not a torrent list")]);
    assert!(qbittorrent.seeding().await.is_err());
}

#[tokio::test]
async fn a_torrent_let_go_is_addressed_by_hash_and_read_back_as_gone() {
    let (qbittorrent, http) = client(vec![(200, ONE_COMPLETED), ACCEPTED, NONE]);
    assert!(qbittorrent.stop_seeding("Show.S01E01").await.is_ok());
    assert_eq!(
        asked(&http).get(1),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/torrents/delete"),
            Some("hashes=a1&deleteFiles=true".to_owned())
        ))
    );
}

#[tokio::test]
async fn a_client_still_holding_it_afterwards_is_a_failure_rather_than_a_removal() {
    let (qbittorrent, _) = client(vec![(200, ONE_COMPLETED), ACCEPTED, (200, ONE_COMPLETED)]);
    assert!(matches!(
        qbittorrent.stop_seeding("Show.S01E01").await,
        Err(Failure::Refused { detail, .. }) if detail.contains("still holding")
    ));
}

#[tokio::test]
async fn a_name_held_never_or_twice_is_refused_rather_than_guessed_at() {
    for listing in [NONE, (200, TWO_OF_A_NAME)] {
        let (qbittorrent, http) = client(vec![listing]);
        assert!(qbittorrent.stop_seeding("Show.S01E01").await.is_err());
        assert_eq!(http.requests().len(), 1);
    }
}

#[tokio::test]
async fn a_removal_the_client_refuses_is_reported_rather_than_read_back() {
    let (qbittorrent, _) = client(vec![(200, ONE_COMPLETED), (500, "boom")]);
    assert!(qbittorrent.stop_seeding("Show.S01E01").await.is_err());
}

#[test]
fn a_ratio_is_whole_hundredths_and_nothing_downloaded_is_the_largest() {
    assert_eq!(hundredths(1750, 1000), 175);
    assert_eq!(hundredths(1, 0), u32::MAX);
    assert_eq!(hundredths(u64::MAX, 1), u32::MAX);
}
