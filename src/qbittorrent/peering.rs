use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Peering};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Qbittorrent, PREFERENCES};

const UNFORWARDED: u16 = 0;

const LISTEN_PORT: &str = "listen_port";

#[derive(Deserialize)]
struct Listening {
    #[serde(default)]
    listen_port: u16,
}

impl Qbittorrent {
    async fn listening(&self) -> Result<u16, Failure> {
        let listening: Listening = self
            .read(PREFERENCES, "the port it takes peers on could not be read")
            .await?;
        Ok(listening.listen_port)
    }
}

#[async_trait]
impl Peering for Qbittorrent {
    async fn peer_port(&self) -> Result<Option<u16>, Failure> {
        let port = self.listening().await?;
        Ok((port != UNFORWARDED).then_some(port))
    }

    async fn set_peer_port(&self, port: Option<u16>) -> Result<(), Failure> {
        let port = port.unwrap_or(UNFORWARDED);
        self.prefer(Map::from_iter([(
            LISTEN_PORT.to_owned(),
            Value::from(port),
        )]))
        .await?;
        match self.listening().await? {
            listening if listening == port => Ok(()),
            listening => Err(self.endpoint.refused(&format!(
                "the port was set to {port} and the client is listening on {listening}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests;
