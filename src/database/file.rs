use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::PathBuf;
use dirs::home_dir;
use std::io::SeekFrom;
use std::io::prelude::*;

use std::io::{Error, ErrorKind, BufReader, BufWriter};
use std::collections::HashMap;
use std::time::SystemTime;

use crate::score::Deck;
static CONFIG: &'static str = "card-counter.config";
static HISTORY: &'static str = "history.json";

/// Returns the path for the main directory
fn main_dir() -> std::io::Result<PathBuf>{
  match home_dir(){
    Some(mut path) => {
      path.push(".card-counter");
      Ok(path)
    }
    None => Err(Error::new(ErrorKind::Other, "Unable to create path ~/.card-counter."))
  }
}

/// Attempts to create the main folder where card-counter stores it's config and file history, '~/.card-counter'
fn create_main_dir() -> std::io::Result<PathBuf>{
  let path = main_dir()?;
  if path.exists() && path.is_dir(){
    Ok(path)
  } else if path.exists() && path.is_file(){
    Err(Error::new(ErrorKind::Other, "Unable to find home directory."))
  }else{
    fs::create_dir(path.clone())?;
    Ok(path)
  }
}
/// Create or Opens a file handle for `name`
fn get_file(name: &str) -> std::io::Result<File>{
  let mut path = create_main_dir()?;
  path.push(name);

  OpenOptions::new()
    .write(true)
    .read(true)
    .create(true)
    .open(path)
}

/// Updates or creates a local database and inserts the current set of decks as an entry
///  under board_id, given the current time stamp.
/// Ex
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
pub fn update_local_database(board_id: &str, decks: &[Deck]) -> std::io::Result<()>{
  let history = history()?;
  let reader = BufReader::new(&history);
  let mut writer = BufWriter::new(&history);

  // Read from database
  let mut db: HashMap<String, HashMap<u64, Vec<Deck>>> = match serde_json::from_reader(reader){
    Ok(db) => db,
    Err(_) => HashMap::new()
  };

  // Generate a new entry and update the database
  let unix_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
    Ok(n) => n.as_secs(),
    Err(_) => panic!("Unable to get current UNIX time"),
  };
  add_entry(&mut db, board_id, unix_time, decks);

  // Save the json database to file
  let json = match serde_json::to_string(&db) {
    Ok(json) => json,
    Err(err) => panic!("{}", err)
  };
  // Need to reset seek position due to reader setting it to EOF
  writer.seek(SeekFrom::Start(0));
  writer.write_all(json.as_bytes())?;
  Ok(())
}


/// Adds an entry into the database using the board_id and timestamp as keys.
/// If no board_id entry currently exists it creates one and initiates it with
/// current timestamp and list of decks as its first entry.
fn add_entry(db: &mut HashMap<String, HashMap<u64, Vec<Deck>>>, board_id: &str, timestamp: u64, decks: &[Deck]){
  match db.get_mut(board_id){
    Some(timestamps) => {
      timestamps.insert(timestamp, decks.to_vec());
    },
    None => {
      let mut timestamps = HashMap::new();
      timestamps.insert(timestamp, decks.to_vec());
      db.insert(board_id.to_string(), timestamps);
    }
  };
}
/// Opens and returns file handle for the config file. If no file is found it creates ones.
pub fn config() -> std::io::Result<File>{
  get_file(CONFIG)
}

/// Opens and returns the file handle for the history file. If no file is found it creates ones.
pub fn history() -> std::io::Result<File>{
  get_file(HISTORY)
}
