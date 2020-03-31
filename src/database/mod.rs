use crate::errors::*;
use crate::score::Deck;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
pub mod aws;
pub mod config;
pub mod file;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Entry {
  board_name: String,
  time_stamp: u64,
  decks: Vec<Deck>,
}

type Entries = Vec<Entry>;

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
      board_name: "Default".to_string(),
      time_stamp: 0,
      decks: Vec::new(),
    }
  }
}

trait Database {
  fn init() -> Result<()>;
  fn add_entry(self, entry: Entry) -> Result<()>;
  fn all_entries(self) -> Result<Entries>;
  fn get_entry(self, board_name: String, time_stamp: u64) -> Result<Entry>;
  fn query_entries(self, board_name: String, time_stamp: u64) -> Result<Entries>;
}
