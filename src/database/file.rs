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
static DATABASE: &'static str = "database.json";

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

/// Opens and returns file handle for the config file. If no file is found it creates ones.
fn config_file() -> std::io::Result<File>{
  get_file(CONFIG)
}

/// Opens and returns the file handle for the history file. If no file is found it creates ones.
fn database_file() -> std::io::Result<File>{
  get_file(DATABASE)
}

pub fn get_database() -> std::io::Result<HashMap<String, HashMap<u64, Vec<Deck>>>>{
  let database = database_file()?;
  let reader = BufReader::new(&database);

  match serde_json::from_reader(reader){
    Ok(db) => Ok(db),
    Err(_) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Can not read file as JSON"))
  }
}

pub fn save_database(db: HashMap<String, HashMap<u64, Vec<Deck>>>) -> std::io::Result<()>{
  // Two different error types can occur here. io::Error and JSON serialization error
  let mut writer = BufWriter::new(database_file()?);

    // Save the json database to file
  let json = match serde_json::to_string(&db) {
    Ok(json) => json,
    // TODO: Handle error type and convert to a single error
    Err(err) => panic!("{}", err)
  };
  writer.seek(SeekFrom::Start(0))?;
  writer.write_all(json.as_bytes())?;
  Ok(())
}

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
pub fn update_local_database(board_id: &str, decks: &[Deck]) -> std::io::Result<()>{
  let mut db = get_database()?;
  // Read from database

  // Generate a new entry and update the database
  let unix_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
    Ok(n) => n.as_secs(),
    Err(_) => panic!("Unable to get current UNIX time"),
  };
  add_entry(&mut db, board_id, unix_time, decks);
  // Need to reset seek position due to reader setting it to EOF
  save_database(db)?;
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

pub fn get_latest_entry(
  db: HashMap<String, HashMap<u64, Vec<Deck>>>,
  board_id: &str,
) -> std::io::Result<Vec<Deck>> {
  let board = match db.get(board_id) {
    Some(board) => board,
    None => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                           format!("Can not find board with id {}", board_id).to_string()))
  };
  let mut max = 0u64;

  for key in board.keys(){
    if key.clone() > max{
      max = key.clone();
    }
  }

  match board.get(&max){
    Some(decks) => Ok(decks.to_vec()),
    None => Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                           format!("Can not find board with id {}", board_id).to_string()))
  }
}
