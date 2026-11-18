//! The manifest, as the core at the pinned commit reads it, and the addresses qBittorrent
//! is reached by.

use toml::{Table, Value};

#[test]
fn the_pinned_core_refuses_nothing_in_the_manifest() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/plugin.toml"))
        .unwrap_or_default();
    let read: Result<lemonfiber_plugin::Manifest, _> = toml::from_str(&text);
    let refused: Vec<String> = match read {
        Ok(manifest) => lemonfiber_plugin::refusals(&manifest, &[])
            .into_iter()
            .map(|violation| format!("{}: {}", violation.location, violation.message))
            .collect(),
        Err(why) => vec![why.to_string()],
    };
    assert!(refused.is_empty(), "{refused:#?}");
}

fn manifest() -> Table {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/plugin.toml"))
        .unwrap_or_default()
        .parse()
        .unwrap_or_default()
}

#[test]
fn every_address_qbittorrent_is_reached_by_names_the_port_its_web_ui_listens_on() {
    let manifest = manifest();
    let qbittorrent = manifest
        .get("service")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
        .find(|service| service.get("id").and_then(Value::as_str) == Some("qbittorrent"));
    let named = qbittorrent.map(|service| {
        [
            service.get("listens"),
            service.get("port"),
            service.get("env").and_then(|env| env.get("WEBUI_PORT")),
        ]
        .map(|port| port.and_then(Value::as_integer))
    });
    let addressed = qbittorrent_adapter::adapter::ADDRESS
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<i64>().ok());
    assert!(addressed.is_some());
    assert_eq!(named, Some([addressed; 3]));
}

#[test]
fn the_starting_configuration_keeps_host_header_validation_on() {
    let configuration = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/config/qbittorrent/qBittorrent/qBittorrent.conf"
    ))
    .unwrap_or_default();
    let validation: Vec<&str> = configuration
        .lines()
        .filter(|line| line.starts_with("WebUI\\HostHeaderValidation="))
        .collect();
    assert_eq!(validation, ["WebUI\\HostHeaderValidation=true"]);
}
