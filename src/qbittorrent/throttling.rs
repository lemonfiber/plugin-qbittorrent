use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Failure, Hours, Metering, Moved, Rates, Throttled, Throttling, Wanted, Window,
};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Qbittorrent, PREFERENCES};

const EVERY_DAY: u8 = 0;

const NO_LIMIT: u64 = 0;

const LAST_HOUR: u8 = 23;

const LAST_MINUTE: u8 = 59;

const LIMITS_MODE: &str = "/transfer/speedLimitsMode";

const TRANSFER: &str = "/transfer/info";

const ALTERNATIVE: &str = "1";

#[derive(Deserialize)]
struct Limits {
    #[serde(default)]
    dl_limit: i64,
    #[serde(default)]
    up_limit: i64,
    #[serde(default)]
    alt_dl_limit: i64,
    #[serde(default)]
    alt_up_limit: i64,
    #[serde(default)]
    scheduler_enabled: bool,
}

#[derive(Deserialize)]
struct Moving {
    #[serde(default)]
    dl_info_speed: u64,
    #[serde(default)]
    up_info_speed: u64,
    #[serde(default)]
    dl_info_data: u64,
    #[serde(default)]
    up_info_data: u64,
}

impl Qbittorrent {
    async fn on_the_alternative(&self) -> Result<bool, Failure> {
        let response = self.asked(Method::Get, LIMITS_MODE, None).await?;
        self.endpoint.expect_success(&response)?;
        Ok(response.body.trim() == ALTERNATIVE)
    }

    async fn transfer(&self) -> Result<Moving, Failure> {
        self.read(TRANSFER, "the transfer figures could not be read")
            .await
    }
}

#[async_trait]
impl Throttling for Qbittorrent {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        let held: Limits = self
            .read(PREFERENCES, "the rate limits could not be read")
            .await?;
        let alternative = self.on_the_alternative().await?;
        let (down, up) = if alternative {
            (held.alt_dl_limit, held.alt_up_limit)
        } else {
            (held.dl_limit, held.up_limit)
        };
        Ok(Throttled {
            rates: Rates {
                down: holding(down),
                up: holding(up),
            },
            uploads: true,
            hours: held.scheduler_enabled.then_some(if alternative {
                Hours::Active
            } else {
                Hours::Quiet
            }),
        })
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        if !wanted.window.is_none_or(on_the_clock) {
            return Err(self
                .endpoint
                .refused("that window is not hours and minutes of a day"));
        }
        self.prefer(preferences_of(wanted)).await?;
        self.throttled().await
    }

    async fn moving(&self) -> Result<Rates, Failure> {
        let moving = self.transfer().await?;
        Ok(Rates {
            down: Some(moving.dl_info_speed),
            up: Some(moving.up_info_speed),
        })
    }
}

#[async_trait]
impl Metering for Qbittorrent {
    async fn moved(&self, _month: &str) -> Result<Moved, Failure> {
        let moved = self.transfer().await?;
        Ok(Moved {
            down: moved.dl_info_data,
            up: moved.up_info_data,
            since_start: true,
        })
    }
}

fn preferences_of(wanted: &Wanted) -> Map<String, Value> {
    let (ordinary, alternative) = match wanted.window {
        Some(_) => (wanted.quiet, wanted.active),
        None => (wanted.active, wanted.active),
    };
    let mut asked = Map::from_iter([
        ("dl_limit".to_owned(), figure(ordinary.down).into()),
        ("up_limit".to_owned(), figure(ordinary.up).into()),
        ("alt_dl_limit".to_owned(), figure(alternative.down).into()),
        ("alt_up_limit".to_owned(), figure(alternative.up).into()),
        (
            "scheduler_enabled".to_owned(),
            wanted.window.is_some().into(),
        ),
    ]);
    if let Some(window) = wanted.window {
        asked.extend([
            ("schedule_from_hour".to_owned(), window.from_hour.into()),
            ("schedule_from_min".to_owned(), window.from_minute.into()),
            ("schedule_to_hour".to_owned(), window.to_hour.into()),
            ("schedule_to_min".to_owned(), window.to_minute.into()),
            ("scheduler_days".to_owned(), EVERY_DAY.into()),
        ]);
    }
    asked
}

fn on_the_clock(window: Window) -> bool {
    window.from_hour <= LAST_HOUR
        && window.to_hour <= LAST_HOUR
        && window.from_minute <= LAST_MINUTE
        && window.to_minute <= LAST_MINUTE
}

fn holding(figure: i64) -> Option<u64> {
    u64::try_from(figure).ok().filter(|held| *held > NO_LIMIT)
}

fn figure(limit: Option<u64>) -> i64 {
    limit.map_or(0, |bytes| i64::try_from(bytes).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests;
