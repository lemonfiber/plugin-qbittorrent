//! A sign-in in each shape qBittorrent answers one, asked through the client the adapter
//! runs, behind the host-header validation qBittorrent keeps on.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use lemonfiber_adapter_kit::web::Web;
use lemonfiber_ports::service::{Download, Failure, Transfers};
use qbittorrent_adapter::credential::Credential;
use qbittorrent_adapter::qbittorrent::Qbittorrent;
use tokio::net::TcpListener;

const SESSION: &str = "a-session";

const USERNAME: &str = "admin";

const PASSWORD: &str = "a-password";

#[derive(Clone, Copy)]
enum Shape {
    Taken,
    NoContent,
}

#[derive(Clone, Copy)]
struct WebUi {
    shape: Shape,
    port: u16,
}

impl WebUi {
    fn cookie(self) -> String {
        match self.shape {
            Shape::Taken => "SID".to_owned(),
            Shape::NoContent => format!("QBT_SID_{}", self.port),
        }
    }

    fn validated(self, headers: &HeaderMap) -> bool {
        headers
            .get(header::HOST)
            .and_then(|host| host.to_str().ok())
            .and_then(|host| host.rsplit_once(':'))
            .and_then(|(_, port)| port.parse::<u16>().ok())
            == Some(self.port)
    }

    fn holds_the_session(self, headers: &HeaderMap) -> bool {
        let carried = format!("{}={SESSION}", self.cookie());
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .any(|pair| pair.trim() == carried)
    }
}

async fn login(State(web_ui): State<WebUi>, headers: HeaderMap, form: String) -> Response {
    if !web_ui.validated(&headers) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    if form != format!("username={USERNAME}&password={PASSWORD}") {
        return (StatusCode::OK, "Fails.").into_response();
    }
    let cookie = format!(
        "{}={SESSION}; HttpOnly; SameSite=Lax; path=/",
        web_ui.cookie()
    );
    match web_ui.shape {
        Shape::Taken => (StatusCode::OK, [(header::SET_COOKIE, cookie)], "Ok.").into_response(),
        Shape::NoContent => {
            (StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]).into_response()
        }
    }
}

async fn torrents(State(web_ui): State<WebUi>, headers: HeaderMap) -> Response {
    if !web_ui.validated(&headers) {
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    } else if web_ui.holds_the_session(&headers) {
        ([(header::CONTENT_TYPE, "application/json")], "[]").into_response()
    } else {
        (StatusCode::FORBIDDEN, "Forbidden").into_response()
    }
}

async fn asked(shape: Shape, web_ui_port: Option<u16>) -> Option<Result<Vec<Download>, Failure>> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let port = listener.local_addr().ok()?.port();
    let web_ui = WebUi {
        shape,
        port: web_ui_port.unwrap_or(port),
    };
    let app = Router::new()
        .route("/api/v2/auth/login", post(login))
        .route("/api/v2/torrents/info", get(torrents))
        .with_state(web_ui);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let qbittorrent = Qbittorrent::new(
        Arc::new(Web::keeping_sessions().ok()?),
        format!("http://127.0.0.1:{port}"),
        Arc::new(Credential {
            username: USERNAME.to_owned(),
            password: PASSWORD.to_owned(),
        }),
    );
    Some(qbittorrent.transfers().await)
}

#[tokio::test]
async fn a_sign_in_answered_ok_with_sid_opens_the_session_the_next_calls_carry() {
    let transfers = asked(Shape::Taken, None).await;
    assert!(
        matches!(&transfers, Some(Ok(none)) if none.is_empty()),
        "{transfers:?}"
    );
}

#[tokio::test]
async fn a_sign_in_answered_with_no_content_and_a_cookie_named_for_the_port_opens_it_too() {
    let transfers = asked(Shape::NoContent, None).await;
    assert!(
        matches!(&transfers, Some(Ok(none)) if none.is_empty()),
        "{transfers:?}"
    );
}

#[tokio::test]
async fn a_host_naming_another_port_than_the_web_ui_listens_on_is_refused_as_unauthorised() {
    for shape in [Shape::Taken, Shape::NoContent] {
        let transfers = asked(shape, Some(1)).await;
        assert!(
            matches!(transfers, Some(Err(Failure::Unauthorised { .. }))),
            "{transfers:?}"
        );
    }
}
