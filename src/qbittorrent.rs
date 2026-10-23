//! qBittorrent, the torrent client, asked through its web UI API signed in as the account
//! lemonfiber wrote for this adapter.

use std::sync::Arc;

use lemonfiber_adapter_kit::endpoint::{describe, form_encoded, Endpoint};
use lemonfiber_ports::http::{Http, Method, Request, Response};
use lemonfiber_ports::service::Failure;
use serde_json::{Map, Value};

use crate::credential::{Credential, Holding};

mod fetching;
mod filing;
mod peering;
mod seeding;
mod throttling;
mod transfers;

/// What every failure is blamed on.
pub const SERVICE: &str = "qbittorrent";

const API: &str = "/api/v2";

const LOGIN: &str = "/auth/login";

const VERSION: &str = "/app/version";

const PREFERENCES: &str = "/app/preferences";

const SET_PREFERENCES: &str = "/app/setPreferences";

const TORRENTS: &str = "/torrents/info";

const SIGNED_OUT: u16 = 403;

const SIGNED_IN: u16 = 204;

const REFUSED: u16 = 401;

const TAKEN: &str = "Ok.";

const FAILS: &str = "Fails.";

const CONTENT_TYPE: &str = "Content-Type";

const FORM: &str = "application/x-www-form-urlencoded";

/// A client for one qBittorrent, signed in to with the credential `held` supplies.
pub struct Qbittorrent {
    endpoint: Endpoint,
    held: Arc<dyn Holding>,
}

impl Qbittorrent {
    /// The qBittorrent reached at `base` over `http`, signed in to with what `held`
    /// supplies.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, held: Arc<dyn Holding>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
            held,
        }
    }

    /// Whether qBittorrent's web UI answers, signed in or not.
    pub async fn answers(&self) -> bool {
        self.endpoint
            .send(&self.request(Method::Get, VERSION, None))
            .await
            .is_ok_and(|response| response.is_success() || response.status == SIGNED_OUT)
    }

    fn credential(&self) -> Result<Credential, Failure> {
        self.held.credential().map_err(|why| {
            self.endpoint.refused(&format!(
                "this adapter holds no credential for qBittorrent: {why}"
            ))
        })
    }

    fn request(&self, method: Method, path: &str, form: Option<&[(&str, &str)]>) -> Request {
        Request {
            method,
            url: self.endpoint.url(&format!("{API}{path}")),
            headers: form
                .map(|_| (CONTENT_TYPE.to_owned(), FORM.to_owned()))
                .into_iter()
                .collect(),
            body: form.map(form_encoded),
            pinned: None,
        }
    }

    async fn login(&self, credential: &Credential) -> Result<(), Failure> {
        let request = self.request(
            Method::Post,
            LOGIN,
            Some(&[
                ("username", credential.username.as_str()),
                ("password", credential.password.as_str()),
            ]),
        );
        let response = self.endpoint.send(&request).await?;
        if response.status == SIGNED_IN || (response.is_success() && response.body.trim() == TAKEN)
        {
            Ok(())
        } else if [SIGNED_OUT, REFUSED].contains(&response.status) || response.body.trim() == FAILS
        {
            Err(self.endpoint.unauthorised())
        } else {
            Err(self.endpoint.refused(&describe(&response)))
        }
    }

    async fn asked(
        &self,
        method: Method,
        path: &str,
        form: Option<&[(&str, &str)]>,
    ) -> Result<Response, Failure> {
        let credential = self.credential()?;
        let request = self.request(method, path, form);
        let response = self.endpoint.send(&request).await?;
        if response.status != SIGNED_OUT {
            return Ok(response);
        }
        self.login(&credential).await?;
        self.endpoint.send(&request).await
    }

    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        whenever: &str,
    ) -> Result<T, Failure> {
        let response = self.asked(Method::Get, path, None).await?;
        self.endpoint.decode(&response, whenever)
    }

    async fn wrote(&self, path: &str, form: &[(&str, &str)]) -> Result<(), Failure> {
        let response = self.asked(Method::Post, path, Some(form)).await?;
        self.endpoint.expect_success(&response)
    }

    async fn prefer(&self, preferences: Map<String, Value>) -> Result<(), Failure> {
        let asked = Value::Object(preferences).to_string();
        self.wrote(SET_PREFERENCES, &[("json", &asked)]).await
    }
}

#[cfg(test)]
mod tests;
