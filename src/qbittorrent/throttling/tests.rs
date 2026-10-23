use lemonfiber_ports::service::{Failure, Hours, Metering, Rates, Throttling, Wanted, Window};
use serde_json::json;

use super::super::tests::{asked, client};
use super::{figure, holding, preferences_of};

const SCHEDULED: &str = r#"{
    "dl_limit":0,"up_limit":0,
    "alt_dl_limit":5242880,"alt_up_limit":262144,
    "scheduler_enabled":true
}"#;

const FLAT: &str = r#"{
    "dl_limit":1048576,"up_limit":-1,
    "alt_dl_limit":0,"alt_up_limit":0,
    "scheduler_enabled":false
}"#;

const TRANSFER: &str =
    r#"{"dl_info_speed":2048,"up_info_speed":512,"dl_info_data":900,"up_info_data":80}"#;

const DONE: (u16, &str) = (200, "");

fn a_day() -> Window {
    Window {
        from_hour: 7,
        from_minute: 0,
        to_hour: 23,
        to_minute: 30,
    }
}

fn a_household() -> Wanted {
    Wanted {
        active: Rates {
            down: Some(5_242_880),
            up: Some(262_144),
        },
        quiet: Rates::default(),
        window: Some(a_day()),
    }
}

#[tokio::test]
async fn a_client_inside_its_scheduled_window_reports_the_alternative_limits() {
    let held = client(vec![(200, SCHEDULED), (200, "1")])
        .0
        .throttled()
        .await;
    assert!(held.is_ok_and(|held| held.hours == Some(Hours::Active)
        && held.rates.down == Some(5_242_880)
        && held.rates.up == Some(262_144)
        && held.uploads));
}

#[tokio::test]
async fn the_same_client_outside_that_window_reports_the_ordinary_ones() {
    let held = client(vec![(200, SCHEDULED), (200, "0")])
        .0
        .throttled()
        .await;
    assert!(
        held.is_ok_and(|held| held.hours == Some(Hours::Quiet) && held.rates == Rates::default())
    );
}

#[tokio::test]
async fn a_client_keeping_no_schedule_is_on_neither_side_of_the_day() {
    let held = client(vec![(200, FLAT), (200, "0")]).0.throttled().await;
    assert!(held.is_ok_and(|held| held.hours.is_none()
        && held.rates.down == Some(1_048_576)
        && held.rates.up.is_none()));
}

#[tokio::test]
async fn a_limit_is_written_in_one_preference_and_read_back_from_the_client() {
    let (qbittorrent, http) = client(vec![DONE, (200, SCHEDULED), (200, "1")]);
    let held = qbittorrent.restrain(&a_household()).await;
    assert!(held.is_ok_and(|held| held.rates.down == Some(5_242_880)));
    assert!(asked(&http)
        .first()
        .is_some_and(|(_, url, _)| url.ends_with("/api/v2/app/setPreferences")));
}

#[tokio::test]
async fn a_window_off_the_clock_is_refused_before_anything_is_asked() {
    for window in [
        Window {
            from_hour: 24,
            ..a_day()
        },
        Window {
            to_hour: 99,
            ..a_day()
        },
        Window {
            from_minute: 60,
            ..a_day()
        },
        Window {
            to_minute: 255,
            ..a_day()
        },
    ] {
        let (qbittorrent, http) = client(Vec::new());
        let refused = qbittorrent
            .restrain(&Wanted {
                window: Some(window),
                ..a_household()
            })
            .await;
        assert!(matches!(refused, Err(Failure::Refused { .. })));
        assert!(http.requests().is_empty());
    }
}

#[tokio::test]
async fn a_client_that_refuses_the_write_is_a_failure_rather_than_a_silent_success() {
    assert!(client(vec![(500, "no")])
        .0
        .restrain(&a_household())
        .await
        .is_err());
}

#[tokio::test]
async fn what_it_is_moving_and_what_it_has_moved_come_off_one_read() {
    assert_eq!(
        client(vec![(200, TRANSFER)]).0.moving().await.ok(),
        Some(Rates {
            down: Some(2048),
            up: Some(512)
        })
    );
    let moved = client(vec![(200, TRANSFER)]).0.moved("2026-09").await;
    assert!(moved.is_ok_and(|moved| moved.down == 900 && moved.up == 80 && moved.since_start));
}

#[tokio::test]
async fn a_client_whose_preferences_or_figures_will_not_read_is_a_failure() {
    assert!(client(vec![(200, "not json")]).0.throttled().await.is_err());
    assert!(client(vec![(200, SCHEDULED), (500, "boom")])
        .0
        .throttled()
        .await
        .is_err());
    assert!(client(vec![(200, "not json")]).0.moving().await.is_err());
}

#[test]
fn a_window_writes_both_pairs_of_limits_and_the_schedule_every_day() {
    assert_eq!(
        serde_json::Value::Object(preferences_of(&a_household())),
        json!({
            "dl_limit": 0, "up_limit": 0,
            "alt_dl_limit": 5_242_880, "alt_up_limit": 262_144,
            "scheduler_enabled": true,
            "schedule_from_hour": 7, "schedule_from_min": 0,
            "schedule_to_hour": 23, "schedule_to_min": 30,
            "scheduler_days": 0
        })
    );
}

#[test]
fn no_window_holds_both_pairs_to_the_active_rates_and_switches_the_schedule_off() {
    assert_eq!(
        serde_json::Value::Object(preferences_of(&Wanted {
            window: None,
            ..a_household()
        })),
        json!({
            "dl_limit": 5_242_880, "up_limit": 262_144,
            "alt_dl_limit": 5_242_880, "alt_up_limit": 262_144,
            "scheduler_enabled": false
        })
    );
}

#[test]
fn the_clients_zero_is_no_limit_and_a_limit_too_large_is_the_largest_it_holds() {
    assert_eq!(holding(0), None);
    assert_eq!(holding(-1), None);
    assert_eq!(holding(1_048_576), Some(1_048_576));
    assert_eq!(figure(None), 0);
    assert_eq!(figure(Some(1_048_576)), 1_048_576);
    assert_eq!(figure(Some(u64::MAX)), i64::MAX);
}
