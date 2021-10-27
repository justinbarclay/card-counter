use crate::{errors::*, score::Deck};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use dialoguer::Select;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt, time::SystemTime};

pub mod aws;
pub mod azure;
pub mod config;
pub mod json;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum DatabaseType {
  Aws,
  Local,
  Azure,
}

impl fmt::Display for DatabaseType {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      DatabaseType::Local => write!(f, "local"),
      DatabaseType::Aws => write!(f, "aws"),
      DatabaseType::Azure => write!(f, "azure"),
    }
  }
}

impl Default for DatabaseType {
  fn default() -> Self {
    DatabaseType::Local
  }
}

fn select_date(keys: &[i64]) -> Option<i64> {
  let rev_keys: Vec<i64> = keys.iter().cloned().rev().collect();
  let items: Vec<String> = rev_keys
    .iter()
    .map(|item| {
      NaiveDateTime::from_timestamp(*item, 0)
        .format("%b %d, %R UTC")
        .to_string()
    })
    .collect();

  match Select::new()
    .with_prompt("Compare board with record at: ")
    .items(&items)
    .max_length(15)
    .default(0)
    .interact()
  {
    Ok(index) => Some(rev_keys[index]),
    Err(_) => None,
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Entry {
  pub board_id: String,
  pub time_stamp: i64,
  pub decks: Vec<Deck>,
}

impl Ord for Entry {
  fn cmp(&self, other: &Self) -> Ordering {
    self.time_stamp.cmp(&other.time_stamp)
  }
}

impl PartialOrd for Entry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl PartialEq for Entry {
  fn eq(&self, other: &Self) -> bool {
    self.time_stamp == other.time_stamp && self.board_id == other.board_id
  }
}

impl Eq for Entry {}

pub type Entries = Vec<Entry>;

// Given a board, the user will be prompted to select an entry based on their timestamps. This can error based on generating prompts to a user.
pub fn get_decks_by_date(entries: Entries) -> Option<Vec<Deck>> {
  let mut keys: Vec<i64> = entries.iter().map(|entry| entry.time_stamp).collect();

  keys.sort_unstable();
  let date;

  if !keys.is_empty() {
    date = select_date(&keys)?;
  } else {
    return None;
  }

  entries
    .iter()
    .find(|entry| entry.time_stamp == date)
    .map(|entry| entry.decks.clone())
}

impl Entry {
  // Gets the current Unix timestamp
  pub fn get_current_timestamp() -> Result<i64> {
    Ok(
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .wrap_err_with(|| "Unable to get UNIX time.")?
        .as_secs() as i64,
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

#[derive(Debug)]
pub struct DateRange {
  pub start: i64,
  pub end: i64,
}

impl DateRange {
  pub fn from_strs(start: &str, end: &str) -> Self {
    let start = NaiveDateTime::parse_from_str(&format!("{} 0:0:0", start), "%F %H:%M:%S")
      .expect("Unable to parse date")
      .timestamp();

    let end = NaiveDateTime::parse_from_str(&format!("{} 0:0:0", end), "%F %H:%M:%S")
      .expect("Unable to parse date")
      .timestamp();

    DateRange { start, end }
  }
}

impl Default for DateRange {
  fn default() -> Self {
    let time = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .unwrap() // Will panic
      .as_secs() as i64;
    DateRange {
      start: time,
      end: time,
    }
  }
}

#[async_trait]
pub trait Database {
  // May mutate self
  async fn add_entry(&self, entry: Entry) -> Result<()>;
  async fn all_entries(&self) -> Result<Option<Entries>>;
  async fn get_entry(&self, board_name: String, time_stamp: i64) -> Result<Option<Entry>>;
  async fn query_entries(
    &self,
    board_name: String,
    date_range: Option<DateRange>,
  ) -> Result<Option<Entries>>;

  fn what_type(&self) -> String;
}
