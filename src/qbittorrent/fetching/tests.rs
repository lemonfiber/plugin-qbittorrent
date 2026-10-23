use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Fetching, Pulling};

use super::super::tests::{asked, client, BASE};

const ONE: &str = r#"[{"name":"Show.S01E01"}]"#;

const NONE: &str = "[]";

const STARTS: &str = r#"{"add_stopped_enabled":false}"#;

const WAITS: &str = r#"{"add_stopped_enabled":true}"#;

const DONE: (u16, &str) = (200, "");

#[tokio::test]
async fn a_client_is_fetching_while_it_runs_something_or_would_start_the_next() {
    for (running, adding, pulling) in [
        (NONE, STARTS, Pulling::Fetching),
        (ONE, WAITS, Pulling::Fetching),
        (NONE, WAITS, Pulling::Stopped),
    ] {
        let (qbittorrent, _) = client(vec![(200, running), (200, adding)]);
        assert_eq!(qbittorrent.pulling().await.ok(), Some(pulling));
    }
}

#[tokio::test]
async fn stopping_it_writes_the_preference_before_it_stops_what_is_running() {
    let (qbittorrent, http) = client(vec![DONE, DONE, (200, NONE), (200, WAITS)]);
    assert_eq!(qbittorrent.stop().await.ok(), Some(Pulling::Stopped));
    let sent = asked(&http);
    assert_eq!(
        sent.first(),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/app/setPreferences"),
            Some("json=%7B%22add_stopped_enabled%22%3Atrue%7D".to_owned())
        ))
    );
    assert_eq!(
        sent.get(1),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/torrents/stop"),
            Some("hashes=all".to_owned())
        ))
    );
}

#[tokio::test]
async fn a_client_that_took_both_writes_and_went_on_running_says_so() {
    let (qbittorrent, _) = client(vec![DONE, DONE, (200, ONE), (200, WAITS)]);
    assert_eq!(qbittorrent.stop().await.ok(), Some(Pulling::Fetching));
}

#[tokio::test]
async fn starting_it_again_lets_new_work_in_as_well_as_what_was_held() {
    let (qbittorrent, http) = client(vec![DONE, DONE, (200, ONE), (200, STARTS)]);
    assert_eq!(qbittorrent.resume().await.ok(), Some(Pulling::Fetching));
    assert!(http.asked_for("torrents/start"));
    assert!(asked(&http).first().is_some_and(
        |(_, _, body)| body.as_deref() == Some("json=%7B%22add_stopped_enabled%22%3Afalse%7D")
    ));
}

#[tokio::test]
async fn a_client_that_refuses_a_write_or_a_read_is_reported_rather_than_guessed_at() {
    assert!(client(vec![(500, "")]).0.stop().await.is_err());
    assert!(client(vec![DONE, (500, "")]).0.stop().await.is_err());
    assert!(client(vec![(500, "")]).0.resume().await.is_err());
    assert!(client(vec![(200, "not json")]).0.pulling().await.is_err());
    assert!(client(vec![(200, NONE), (200, "not json")])
        .0
        .pulling()
        .await
        .is_err());
}
