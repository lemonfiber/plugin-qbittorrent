use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Filed, Filing};

use super::super::tests::{asked, client, BASE};

const CATEGORIES: &str = r#"{
    "tv":{"name":"tv","savePath":"/data/downloads/complete/tv"},
    "movies":{"name":"movies","savePath":"films"},
    "music":{"name":"music","savePath":""}}"#;

const MANAGED: &str =
    r#"{"save_path":"/data/downloads/torrents/complete/","auto_tmm_enabled":true}"#;

const BY_HAND: &str =
    r#"{"save_path":"/data/downloads/torrents/complete","auto_tmm_enabled":false}"#;

const DONE: (u16, &str) = (200, "");

fn filed(category: &str, folder: &str) -> Filed {
    Filed {
        category: category.to_owned(),
        folder: folder.to_owned(),
    }
}

#[tokio::test]
async fn each_category_is_read_as_the_folder_its_torrents_complete_into() {
    let (qbittorrent, _) = client(vec![(200, CATEGORIES), (200, MANAGED)]);
    assert_eq!(
        qbittorrent.filed().await.ok(),
        Some(vec![
            filed("movies", "/data/downloads/torrents/complete/films"),
            filed("music", "/data/downloads/torrents/complete/music"),
            filed("tv", "/data/downloads/complete/tv"),
        ])
    );
}

#[tokio::test]
async fn torrents_managed_by_hand_are_read_as_completing_into_the_default_folder() {
    let (qbittorrent, _) = client(vec![(200, CATEGORIES), (200, BY_HAND)]);
    let held = qbittorrent.filed().await.unwrap_or_default();
    assert_eq!(held.len(), 3);
    assert!(held
        .iter()
        .all(|one| one.folder == "/data/downloads/torrents/complete"));
}

#[tokio::test]
async fn categories_or_preferences_that_will_not_read_are_a_failure() {
    assert!(client(vec![(200, "not json")]).0.filed().await.is_err());
    assert!(client(vec![(200, CATEGORIES), (500, "")])
        .0
        .filed()
        .await
        .is_err());
}

#[tokio::test]
async fn a_held_category_is_edited_a_new_one_made_and_automatic_management_switched_on() {
    let (qbittorrent, http) = client(vec![
        (200, r#"{"tv":{"name":"tv","savePath":""}}"#),
        DONE,
        DONE,
        (200, BY_HAND),
        DONE,
    ]);
    let written = qbittorrent
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;
    assert_eq!(written.ok(), Some(()));
    let sent = asked(&http);
    assert_eq!(
        sent.get(1),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/torrents/editCategory"),
            Some("category=tv&savePath=%2Fdata%2Fdownloads%2Fcomplete%2Ftv".to_owned())
        ))
    );
    assert_eq!(
        sent.get(2),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/torrents/createCategory"),
            Some("category=music&savePath=%2Fdata%2Fdownloads%2Fcomplete%2Fmusic".to_owned())
        ))
    );
    assert_eq!(
        sent.last(),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/app/setPreferences"),
            Some("json=%7B%22auto_tmm_enabled%22%3Atrue%7D".to_owned())
        ))
    );
}

#[tokio::test]
async fn automatic_management_already_on_is_left_alone() {
    let (qbittorrent, http) = client(vec![(200, "{}"), DONE, (200, MANAGED)]);
    let written = qbittorrent
        .file_under(&[filed("tv", "/data/downloads/complete/tv")])
        .await;
    assert_eq!(written.ok(), Some(()));
    assert!(!http.asked_for("setPreferences"));
}

#[tokio::test]
async fn a_category_the_client_refuses_stops_the_filing_and_says_which() {
    let (qbittorrent, http) = client(vec![(200, "{}"), (409, "")]);
    let written = qbittorrent
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;
    assert!(matches!(
        written,
        Err(Failure::Refused { detail, .. }) if detail.contains("tv")
    ));
    assert_eq!(http.requests().len(), 2);
}

#[tokio::test]
async fn a_category_or_folder_qbittorrent_could_not_hold_is_refused_before_anything_is_asked() {
    for (category, folder) in [
        ("", "/data/downloads/complete/tv"),
        ("tv&x=y", "/data/downloads/complete/tv"),
        ("tv", "data/downloads/complete/tv"),
        ("tv", "/data/../config"),
        ("tv", "/data/downloads/complete/tv\r"),
    ] {
        let (qbittorrent, http) = client(vec![(200, "{}")]);
        let refused = qbittorrent.file_under(&[filed(category, folder)]).await;
        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains("could hold")),
            "{category:?} {folder:?}: {refused:?}"
        );
        assert!(http.requests().is_empty());
    }
}
