use crate::errors::*;
use crate::score::Deck;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

pub mod aws;
pub mod config;
pub mod file;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Entry {
  pub board_id: String,
  pub time_stamp: u64,
  pub decks: Vec<Deck>,
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
      board_id: "Default".to_string(),
      time_stamp: 0,
      decks: Vec::new(),
    }
  }
}

#[async_trait]
pub trait Database {
  async fn add_entry(&self, entry: Entry) -> Result<()>;
  async fn all_entries(&self) -> Result<Entries>;
  async fn get_entry(&self, board_name: String, time_stamp: u64) -> Result<Option<Entry>>;
  async fn query_entries(
    &self,
    board_name: String,
    time_stamp: Option<u64>,
  ) -> Result<Option<Vec<Deck>>>;
}
