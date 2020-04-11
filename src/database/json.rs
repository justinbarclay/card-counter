use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};
use std::path::PathBuf;
use std::{
  fs,
  fs::{File, OpenOptions},
};

use std::collections::HashMap;

use super::{Database, DateRange, Entries, Entry};
use crate::errors::*;
use crate::score::Deck;
use async_trait::async_trait;
use dirs::home_dir;
static CONFIG: &str = "card-counter.yaml";
static DATABASE: &str = "database.json";

#[derive(Default, Clone)]
pub struct JSON {
  database: HashMap<String, LocalEntry>,
}

pub type LocalEntry = HashMap<u64, Vec<Deck>>;

// This code has a lot of panics in it, I've chosen to do this because where there are panics it's in the case of IO or data errors.
// Such as being unable to open the file, unable to parse the file into json, or being unable to save the file. Unfortunately,
// in all of these cases there is no sane default behavior to save the data or rescue ourselves. Instead I have tried to structure
// the program so that it can only panic at the outer edges, IE the layer of IO. This helps to prevent wrapping all my functions in
// Result enums and leave results to recoverable errors or where one can assume default behavior in error cases.

/// Returns the path for the main directory
fn main_dir() -> PathBuf {
  let mut path = home_dir().expect("Unable to determine Home directory.");
  path.push(".card-counter");
  path
}

// TODO: Deprecate
/// Attempts to create the main folder where card-counter stores it's config and file history, '~/.card-counter'
fn create_main_dir() -> PathBuf {
  let path = main_dir();
  if path.exists() && path.is_dir() {
    path
  } else if path.exists() && path.is_file() {
    panic!("Unable to create directory $HOME/.card-counter because it already exists as a file.")
  } else {
    fs::create_dir(path.clone()).expect("Unable to create .card-counter directory in $HOME");
    path
  }
}

/// Attempts to create the main folder where card-counter stores its config and database files, '~/.card-counter'
#[allow(unused)]
fn find_or_create_main_dir() -> Result<PathBuf> {
  let path = main_dir();

  if !(path.exists() && path.is_dir()) {
    fs::create_dir(path.clone()).chain_err(|| "Unable to create .card-counter directory in $HOME")?;
  }

  Ok(path)
}
/// Create or Opens a file handle for `name`
fn get_file(name: &str) -> Result<File> {
  let mut path = create_main_dir();
  path.push(name);

  Ok(
    OpenOptions::new()
      .write(true)
      .read(true)
      .create(true)
      .open(path)?,
  )
}

// Opens and returns file handle for the config file. If no file is found it creates a one.
pub fn config_file() -> Result<File> {
  get_file(CONFIG)
}

/// Opens and returns the file handle for the history file. If no file is found it creates a new one.
fn database_file() -> Result<File> {
  get_file(DATABASE)
}

#[async_trait]
impl Database for JSON {
  /// Updates or creates a local database and inserts the current set of decks as an entry
  ///  under board_id, given the current time stamp.
  /// Ex:
  /// ```
  /// {
  ///   "56eab922556b7a05c2f3b25e": {
  ///     "1580111037": [
  ///       {
  ///         "name": "This Sprint",
  ///         "size": 7,
  ///         "score": 34,
  ///         "unscored": 2,
  ///         "estimated": 34
  ///       }]
  ///   }
  /// }
  /// ```
  async fn add_entry(&mut self, entry: Entry) -> Result<()> {
    // Adds an entry into the database using the board_id and timestamp as keys.
    // If no board_id entry currently exists it creates one and initiates it with
    // current timestamp and list of decks as its first entry.
    match self.database.get_mut(&entry.board_id) {
      Some(timestamps) => {
        timestamps.insert(entry.time_stamp, entry.decks);
      }
      None => {
        let mut timestamps = HashMap::new();
        timestamps.insert(entry.time_stamp, entry.decks);
        self.database.insert(entry.board_id, timestamps);
      }
    };

    self.save()
  }
  async fn all_entries(&self) -> Result<Option<Entries>> {
    Ok(None)
  }
  async fn get_entry(&self, board_name: String, time_stamp: u64) -> Result<Option<Entry>> {
    let result = match self
      .database
      .get(&board_name)
      .unwrap_or(&HashMap::default())
      .get(&time_stamp)
    {
      Some(item) => Some(Entry {
        board_id: board_name,
        decks: item.clone(),
        time_stamp,
      }),
      None => None,
    };

    Ok(result)
  }

  async fn query_entries(
    &self,
    board_id: String,
    date_range: Option<DateRange>,
  ) -> Result<Option<Entries>> {
    let results = match self.database.get(&board_id) {
      Some(results) => results,
      None => return Ok(None),
    };

    if let Some(range) = date_range {
      let entries: Entries = results
        .iter()
        .fold(Vec::new(), |mut collection, (key, value)| {
          if range.start < *key && *key < range.end {
            collection.push(Entry {
              board_id: board_id.clone(),
              time_stamp: *key,
              decks: value.clone(),
            })
          }
          collection
        });
      Ok(Some(entries))
    } else {
      let entries: Entries = results
        .iter()
        .map(|(key, value)| Entry {
          board_id: board_id.clone(),
          time_stamp: *key,
          decks: value.clone(),
        })
        .collect();
      Ok(Some(entries))
    }
  }
}

impl JSON {
  pub fn init() -> Result<Self> {
    // No Sane default: if we can't get the database we need to error out to the use
    let file = database_file().chain_err(|| "Unable to open database at $HOME/.card-counter")?;
    let reader = BufReader::new(&file);

    // We need to know the length of the file or we could erroneously toss a JSON error.
    // We should error out if we can't read metadata.
    if file
      .metadata()
      .chain_err(|| "Unable to read metadata for $HOME/.card-counter/database.json.")?
      .len()
      == 0
    {
      Ok(JSON::default())
    } else {
      // No Sane default: If we can't parse as json, it might be recoverable and we don't
      // want to overwrite user data
      Ok(JSON {
        database: serde_json::from_reader(reader)
          .chain_err(|| "Unable to parse database file as json")?,
      })
    }
  }

  /// Attempts to save the database and panics if it can't parse the db into JSON or if it can't write to
  /// the database file.
  fn save(&self) -> Result<()> {
    // No Sane default: We want to error if we can't open or access the File handle
    let file = database_file().chain_err(|| "Unable to open database")?;

    // Clear out file before writing to it.
    file.set_len(0)?;
    let mut writer = BufWriter::new(file);
    // There is no safe default behavior we can perform here.
    let json = serde_json::to_string(&self.database).chain_err(|| "Unable to parse database")?;

    // No Sane default: IO Errors if we can't move around the file
    writer
      .seek(SeekFrom::Start(0))
      .chain_err(|| "Unable to write to file $HOME/.card-counter/database.json")?;
    writer
      .write_all(json.as_bytes())
      .chain_err(|| "Unable to write to file $HOME/.card-counter/database.json")?;
    Ok(())
  }
}
