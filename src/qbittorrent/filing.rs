use std::collections::BTreeMap;

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Filed, Filing};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Qbittorrent, PREFERENCES};

const CATEGORIES: &str = "/torrents/categories";

const EDIT: &str = "/torrents/editCategory";

const CREATE: &str = "/torrents/createCategory";

const AUTOMATIC: &str = "auto_tmm_enabled";

#[derive(Deserialize)]
struct Category {
    #[serde(rename = "savePath", default)]
    save_path: String,
}

#[derive(Deserialize)]
struct Saving {
    #[serde(default)]
    save_path: String,
    #[serde(default)]
    auto_tmm_enabled: bool,
}

impl Qbittorrent {
    async fn categories(&self) -> Result<BTreeMap<String, Category>, Failure> {
        self.read(CATEGORIES, "the categories could not be read")
            .await
    }

    async fn saving(&self) -> Result<Saving, Failure> {
        self.read(PREFERENCES, "where torrents are saved could not be read")
            .await
    }
}

#[async_trait]
impl Filing for Qbittorrent {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        let categories = self.categories().await?;
        let saving = self.saving().await?;
        Ok(categories
            .into_iter()
            .map(|(name, category)| Filed {
                folder: folder_of(&saving, &name, &category),
                category: name,
            })
            .collect())
    }

    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        if let Some(misshapen) = folders.iter().find(|one| !held_to_shape(one)) {
            return Err(self.endpoint.refused(&format!(
                "{:?} under {:?} is not a category and folder qBittorrent could hold",
                misshapen.category, misshapen.folder
            )));
        }
        let held = self.categories().await?;
        for one in folders {
            let path = if held.contains_key(&one.category) {
                EDIT
            } else {
                CREATE
            };
            self.wrote(
                path,
                &[
                    ("category", one.category.as_str()),
                    ("savePath", one.folder.as_str()),
                ],
            )
            .await
            .map_err(|_| {
                self.endpoint.refused(&format!(
                    "the client would not file {} under {}",
                    one.category, one.folder
                ))
            })?;
        }
        if self.saving().await?.auto_tmm_enabled {
            return Ok(());
        }
        self.prefer(Map::from_iter([(AUTOMATIC.to_owned(), Value::Bool(true))]))
            .await
    }
}

fn held_to_shape(one: &Filed) -> bool {
    let category = one.category.as_str();
    let named = !category.is_empty()
        && category
            .chars()
            .all(|letter| letter.is_alphanumeric() || matches!(letter, '-' | '_' | '.' | ' '));
    let placed = one.folder.starts_with('/')
        && !one.folder.chars().any(char::is_control)
        && !one.folder.split('/').any(|segment| segment == "..");
    named && placed
}

fn folder_of(saving: &Saving, name: &str, category: &Category) -> String {
    let default = saving.save_path.trim_end_matches('/');
    if !saving.auto_tmm_enabled {
        return default.to_owned();
    }
    match category.save_path.trim() {
        "" => format!("{default}/{name}"),
        own if own.starts_with('/') => own.trim_end_matches('/').to_owned(),
        own => format!("{default}/{own}"),
    }
}

#[cfg(test)]
mod tests;
