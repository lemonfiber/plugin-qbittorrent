use lemonfiber_ports::service::{Failure, Peering};

use super::super::tests::{asked, client};

const DONE: (u16, &str) = (200, "");

#[tokio::test]
async fn the_port_it_takes_peers_on_is_the_one_it_listens_on_and_none_is_zero() {
    let (qbittorrent, _) = client(vec![(200, r#"{"listen_port":51413}"#)]);
    assert_eq!(qbittorrent.peer_port().await.ok(), Some(Some(51413)));
    let (qbittorrent, _) = client(vec![(200, r#"{"listen_port":0}"#)]);
    assert_eq!(qbittorrent.peer_port().await.ok(), Some(None));
    let (qbittorrent, _) = client(vec![(200, "not json")]);
    assert!(qbittorrent.peer_port().await.is_err());
}

#[tokio::test]
async fn a_port_is_set_and_read_back_and_none_sets_zero() {
    for (port, written, read) in [
        (
            Some(51413),
            "json=%7B%22listen_port%22%3A51413%7D",
            r#"{"listen_port":51413}"#,
        ),
        (
            None,
            "json=%7B%22listen_port%22%3A0%7D",
            r#"{"listen_port":0}"#,
        ),
    ] {
        let (qbittorrent, http) = client(vec![DONE, (200, read)]);
        assert!(qbittorrent.set_peer_port(port).await.is_ok());
        assert!(asked(&http)
            .first()
            .is_some_and(|(_, _, body)| body.as_deref() == Some(written)));
    }
}

#[tokio::test]
async fn a_client_still_on_another_port_after_the_write_is_a_failure() {
    let (qbittorrent, _) = client(vec![DONE, (200, r#"{"listen_port":6881}"#)]);
    assert!(matches!(
        qbittorrent.set_peer_port(None).await,
        Err(Failure::Refused { .. })
    ));
    let (refused, _) = client(vec![(500, "")]);
    assert!(refused.set_peer_port(Some(51413)).await.is_err());
}
