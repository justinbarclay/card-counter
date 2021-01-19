use crate::errors::*;
use crate::score::Deck;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use dialoguer::Select;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, convert::TryInto, fmt, time::SystemTime};

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
  let rev_keys: Vec<i64> = keys.into_iter().cloned().rev().collect();
  let items: Vec<String> = rev_keys
    .iter()
    .map(|item| {
      NaiveDateTime::from_timestamp(item.clone().try_into().unwrap(), 0)
        .format("%b %d, %R UTC")
        .to_string()
    })
    .collect();

  match Select::new()
    .with_prompt("Compare board with record at: ")
    .items(&items)
    .paged(true)
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

type Entries = Vec<Entry>;

// Given a board, the user will be prompted to select an entry based on their timestamps. This can error based on generating prompts to a user.
pub fn get_decks_by_date(entries: Entries) -> Option<Vec<Deck>> {
  let mut keys: Vec<i64> = entries.iter().map(|entry| entry.time_stamp).collect();

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
  // Gets the current Unix timestampe
  pub fn get_current_timestamp() -> Result<i64> {
    Ok(
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .chain_err(|| "Unable to get UNIX time.")?
        .as_secs() as i64,
    )
  }
  //
  pub fn calculate_burndown(&self, filter: Option<&str>) -> (i32, i32) {
    self
      .decks
      .iter()
      .fold((0, 0), |(incomplete, complete), deck| -> (i32, i32) {
        if filter.is_some() && deck.list_name.contains(filter.unwrap()) {
          (incomplete, complete)
        } else if deck.list_name.contains("Done") {
          (incomplete, complete + deck.score)
        } else {
          (incomplete + deck.score, complete)
        }
      })
  }
}

pub fn format_to_burndown(entries: Vec<Entry>, filter: Option<&str>) -> Vec<String> {
  let mut entries = entries.to_vec();

  // In some cases, there are going to be multiple entries for a
  // single days when building a burndown chart, we want to use the
  // last entry in that day
  entries.sort();
  let mut burndown: Vec<(String, i32, i32)> = Vec::new();
  for entry in entries {
    let time = NaiveDateTime::from_timestamp(entry.time_stamp, 0)
      .format("%d-%m-%Y")
      .to_string();
    let (incomplete, complete) = entry.calculate_burndown(filter);

    // Remove duplicate entry
    if let Some(entry) = burndown.last() {
      if entry.0 == time {
        burndown.pop();
      }
    }

    burndown.push((time, incomplete, complete));
  }

  //TODO: Make immutable
  let mut output = vec!["Date,Incomplete,Complete".to_string()];
  output.extend(
    burndown
      .iter()
      .map(|(time, incomplete, complete)| format!("{},{},{}", time, incomplete, complete)),
  );
  output
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
}
