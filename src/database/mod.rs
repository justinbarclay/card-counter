use std::time::SystemTime;
use serde::{Serialize, Deserialize};
use crate::errors::*;
use crate::score::Deck;
pub mod config;
pub mod file;
pub mod aws;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Entry{
  board_name: String,
  time_stamp: u64,
  decks: Vec<Deck>
}

type Entries = Vec<Entry>;

impl Entry{
  pub fn get_current_timestamp() -> Result<u64>{
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .chain_err(|| "Unable to get UNIX time.")?.as_secs()
    )
  }
}

impl Default for Entry {
  fn default() -> Self {
    Entry{
      board_name: "Default".to_string(),
      time_stamp: 0,
      decks: Vec::new()
    }
  }
}

