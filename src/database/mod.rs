use crate::errors::*;
use crate::score::Deck;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use dialoguer::Select;
use serde::{Deserialize, Serialize};
use std::{convert::TryInto, time::SystemTime};

pub mod aws;
pub mod config;
pub mod json;

fn select_date(keys: &[u64]) -> Option<u64> {
  let items: Vec<String> = keys
    .iter()
    .map(|item| {
      NaiveDateTime::from_timestamp(item.clone().try_into().unwrap(), 0)
        .format("%b %d, %R UTC")
        .to_string()
    })
    .collect();

  match Select::new()
    .with_prompt("Select a date: ")
    .items(&items)
    .default(0)
    .interact()
  {
    Ok(index) => Some(keys[index]),
    Err(_) => None,
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Entry {
  pub board_id: String,
  pub time_stamp: u64,
  pub decks: Vec<Deck>,
}
type Entries = Vec<Entry>;

// Given a board, the user will be prompted to select an entry based on their timestamps. This can error based on generating prompts to a user.
pub fn get_decks_by_date(entries: Entries) -> Option<Vec<Deck>> {
  let mut keys: Vec<u64> = entries.iter().map(|entry| entry.time_stamp).collect();

  keys.sort();
  let date;

  if keys.len() > 0 {
    date = select_date(&keys)?;
  } else {
    return None;
  }

  match entries.iter().find(|entry| entry.time_stamp == date) {
    Some(entry) => Some(entry.decks.clone()),
    None => None,
  }
}

impl Entry {
  pub fn get_current_timestamp() -> Result<u64> {
    Ok(
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .chain_err(|| "Unable to get UNIX time.")?
        .as_secs(),
    )
  }
}

impl Default for Entry {
  fn default() -> Self {
    Entry {
      board_id: "Default".to_string(),
      // This name is hack around timestamp is a reserved keyword in some databases
      time_stamp: 0,
      decks: Vec::new(),
    }
  }
}

pub struct DateRange {
  start: u64,
  end: u64,
}

impl Default for DateRange {
  fn default() -> Self {
    let time = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .unwrap() // Will panic
      .as_secs();
    DateRange {
      start: time,
      end: time,
    }
  }
}

#[async_trait]
pub trait Database {
  // May mutate self
  async fn add_entry(&mut self, entry: Entry) -> Result<()>;
  async fn all_entries(&self) -> Result<Option<Entries>>;
  async fn get_entry(&self, board_name: String, time_stamp: u64) -> Result<Option<Entry>>;
  async fn query_entries(
    &self,
    board_name: String,
    date_range: Option<DateRange>,
  ) -> Result<Option<Entries>>;
}
