use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Transfers};

use super::Qbittorrent;
use crate::credential::{Credential, Holding};

pub(super) const BASE: &str = "http://127.0.0.1:8081";

pub(super) const IN: (u16, &str) = (200, "Ok.");

pub(super) const OUT: (u16, &str) = (403, "Forbidden");

pub(super) fn a_password() -> String {
    ('c'..='r').collect()
}

pub(super) fn client(replies: Vec<(u16, &'static str)>) -> (Qbittorrent, Arc<Fake>) {
    let http = Fake::in_turn(
        replies
            .into_iter()
            .map(|(status, body)| Answer::reply(status, body))
            .collect(),
    );
    let held = Credential {
        username: "admin".to_owned(),
        password: a_password(),
    };
    (Qbittorrent::new(http.clone(), BASE, Arc::new(held)), http)
}

pub(super) fn asked(http: &Fake) -> Vec<(Method, String, Option<String>)> {
    http.requests()
        .into_iter()
        .map(|one| (one.method, one.url, one.body))
        .collect()
}

struct Nothing;

impl Holding for Nothing {
    fn credential(&self) -> Result<Credential, String> {
        Err("upstream.json could not be read: not found".to_owned())
    }
}

#[tokio::test]
async fn a_session_already_held_is_asked_under_without_signing_in() {
    let (qbittorrent, http) = client(vec![(200, "[]")]);
    assert_eq!(qbittorrent.transfers().await.ok(), Some(Vec::new()));
    assert_eq!(asked(&http).len(), 1);
}

#[tokio::test]
async fn a_session_it_does_not_hold_is_opened_with_the_credential_and_asked_again() {
    let (qbittorrent, http) = client(vec![OUT, IN, (200, "[]")]);
    assert_eq!(qbittorrent.transfers().await.ok(), Some(Vec::new()));
    let sent = asked(&http);
    assert_eq!(
        sent.get(1),
        Some(&(
            Method::Post,
            format!("{BASE}/api/v2/auth/login"),
            Some(format!("username=admin&password={}", a_password()))
        ))
    );
    assert_eq!(
        sent.get(2).map(|(_, url, _)| url.as_str()),
        Some(format!("{BASE}/api/v2/torrents/info?filter=downloading").as_str())
    );
    assert!(http
        .requests()
        .get(1)
        .is_some_and(|login| login.headers.contains(&(
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned()
        ))));
}

#[tokio::test]
async fn the_five_line_signs_in_with_no_content_and_refuses_with_unauthorised() {
    let (signed_in, _) = client(vec![OUT, (204, ""), (200, "[]")]);
    assert!(signed_in.transfers().await.is_ok());
    for refused in [(401, ""), (200, "Fails."), (403, "")] {
        let (qbittorrent, _) = client(vec![OUT, refused]);
        assert!(
            matches!(
                qbittorrent.transfers().await,
                Err(Failure::Unauthorised { .. })
            ),
            "{refused:?}"
        );
    }
    let (other, _) = client(vec![OUT, (200, "")]);
    assert!(matches!(
        other.transfers().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn without_a_credential_nothing_is_asked_and_the_refusal_says_why() {
    let http = Fake::silent();
    let refused = Qbittorrent::new(http.clone(), BASE, Arc::new(Nothing))
        .transfers()
        .await;
    assert!(
        matches!(&refused, Err(Failure::Refused { detail, .. })
            if detail == "this adapter holds no credential for qBittorrent: upstream.json could not be read: not found"),
        "{refused:?}"
    );
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn it_answers_while_its_web_ui_answers_signed_in_or_not() {
    for answer in [
        Answer::reply(200, "v5.2.4"),
        Answer::reply(403, "Forbidden"),
    ] {
        let up = Fake::always(answer);
        assert!(
            Qbittorrent::new(up.clone(), BASE, Arc::new(Nothing))
                .answers()
                .await
        );
        assert!(up
            .requests()
            .iter()
            .all(|one| one.url == format!("{BASE}/api/v2/app/version")));
    }
    for down in [Fake::silent(), Fake::always(Answer::reply(503, ""))] {
        assert!(
            !Qbittorrent::new(down, BASE, Arc::new(Nothing))
                .answers()
                .await
        );
    }
}

#[tokio::test]
async fn a_call_refused_as_unauthorised_is_unauthorised() {
    let (qbittorrent, http) = client(vec![(401, "Unauthorized")]);
    assert!(matches!(
        qbittorrent.transfers().await,
        Err(Failure::Unauthorised { .. })
    ));
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn a_client_that_stops_answering_after_signing_in_is_unavailable() {
    let (qbittorrent, _) = client(vec![OUT, IN]);
    assert!(matches!(
        qbittorrent.transfers().await,
        Err(Failure::Unavailable { .. })
    ));
}
