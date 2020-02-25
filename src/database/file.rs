use std::fs::{File, OpenOptions, create_dir};
use std::path::PathBuf;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};
use std::convert::TryInto;

use std::collections::HashMap;
use std::time::SystemTime;

use dirs::home_dir;
use chrono::NaiveDateTime;
use dialoguer::Select;
use crate::errors::*;
use crate::score::Deck;
static CONFIG: &'static str = "card-counter.yaml";
static DATABASE: &'static str = "database.json";

// This code has a lot of panics in it, I've chosen to do this because where there are panics it's in the case of IO or data errors.
// Such as being unable to open the file, unable to parse the file into json, or being unable to save the file. Unfortunately,
// in all of these cases there is no sane default behavior to save the data or rescue ourselves. Instead I have tried to structure
// the program so that it can only panic at the outer edges, IE the layer of IO. This helps to prevent wrapping all my functions in
// Result enums and leave results to recoverable errors or where one can assume default behavior in error cases.

/// Returns the path for the main directory
fn main_dir() -> PathBuf{
  let mut path = home_dir().expect("Unable to determine Home directory.");
  path.push(".card-counter");
  path
}

/// Attempts to create the main folder where card-counter stores it's config and file history, '~/.card-counter'
fn create_main_dir() -> PathBuf{
  let path = main_dir();
  if path.exists() && path.is_dir(){
    path
  } else if path.exists() && path.is_file(){
    panic!("Unable to create directory $HOME/.card-counter because it already exists as a file.")
  }else{
    create_dir(path.clone()).expect("Unable to create .card-counter directory in $HOME");
    path
  }
}
/// Create or Opens a file handle for `name`
fn get_file(name: &str) -> Result<File>{
  let mut path = create_main_dir();
  path.push(name);

  Ok(OpenOptions::new()
    .write(true)
    .read(true)
    .create(true)
    .open(path)?)
}

/// Opens and returns file handle for the config file. If no file is found it creates a one.
pub fn config_file() -> Result<File>{
  get_file(CONFIG)
}

/// Opens and returns the file handle for the history file. If no file is found it creates a new one.
fn database_file() -> Result<File>{
  get_file(DATABASE)
}

type Database = HashMap<String, HashMap<u64, Vec<Deck>>>;
/// Parses the database file into a hash map.
pub fn get_database() -> Database {
  // No Sane default: if we can't get the database we need to error out to the use
  let database = database_file().expect("Unable to open database at $HOME/.card-counter");
  let reader = BufReader::new(&database);

  // We need to know the length of the file or we could erroneously toss a JSON error.
  // We should error out if we can't read metadata.
  if database.metadata().expect("Unable to read metadata for $HOME/.card-counter/database.json.").len() == 0 {
    return HashMap::new();
  };

  // No Sane default: If we can't parse as json, it might be recoverable and we don't
  // want to overwrite user data
  match serde_json::from_reader(reader){
    Ok(db) => db,
    Err(err) => {
      eprintln!("Unable to parse file as JSON");
      panic!("{}", err);
    }
  }
}

/// Attempts to save the database and panics if it can't parse the db into JSON or if it can't write to
/// the database file.
pub fn save_database(db: Database) -> Result<()>{
  // No Sane default: We want to error if we can't open or access the File handle
  let mut writer = BufWriter::new(database_file().chain_err(|| "Unable to open database")?);

  // There is no safe default behavior we can perform here.
  let json = serde_json::to_string(&db).chain_err(|| "Unable to parse database")?;

  // No Sane default: IO Errors if we can't move around the file
  writer.seek(SeekFrom::Start(0)).chain_err(|| "Unable to write to file $HOME/.card-counter/database.json")?;
  writer.write_all(json.as_bytes()).chain_err(|| "Unable to write to file $HOME/.card-counter/database.json")?;
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

pub fn save_local_database(board_id: &str, decks: &[Deck]) -> Result<()>{
  // No Sane default: if we can't get the database we need to error out to the user
  let mut db = get_database();

  // Generate a new entry and update the database
  // If we can't generate a unix time stamp we can't save entry into database.
  // Is there a better scheme one can come up with to be able to always save data?
  // Timestamps are nice because they have they convey several different properties at once.
  // We can quickly convert to string so users can picks what one they want and we can easily
  // sort these keys by number and determine when the data was entered, without having to write
  // our own compare functions.
  let unix_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).chain_err(|| "Unable to get UNIX time.")?;

  add_entry(&mut db, board_id, unix_time.as_secs(), decks);

  // No Sane default: if we can't get the database we need to error out to the use
  save_database(db)?;
  Ok(())
}

/// Adds an entry into the database using the board_id and timestamp as keys.
/// If no board_id entry currently exists it creates one and initiates it with
/// current timestamp and list of decks as its first entry.
fn add_entry(db: &mut Database, board_id: &str, timestamp: u64, decks: &[Deck]){
  // Note to self: this seems wrong? This is a prime example of where a Result should be returned
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

/// Attempts to get an entry from the database if an entry for a board is found
/// it returns a Vector of the decks.
pub fn get_latest_entry(db: Database, board_id: &str) -> Option<Vec<Deck>> {
  // Return none to reprompt user for another board_id
  let board = db.get(board_id)?;

  let mut max = 0u64;

  for key in board.keys(){
    if key.clone() > max{
      max = *key;
    }
  }

  match board.get(&max){
    Some(decks) => Some(decks.to_vec()),
    None => None
  }
}


fn select_date(database: &HashMap<String, HashMap<u64, Vec<Deck>>>, board_id: &str) -> Option<u64> {
  let board = database.get(board_id)?;

  let mut keys: Vec<u64> = board.keys().map(|key| key.clone()).collect();
  keys.sort();
  let items: Vec<NaiveDateTime> = keys.iter().map(|item| NaiveDateTime::from_timestamp(item.clone().try_into().unwrap(), 0)).collect();
  let index: usize = Select::new()
    .with_prompt("Select a date: ")
    .items(&items)
    .default(0)
    .interact().unwrap();

  Some(keys[index])
}

// This looks like a database error
pub fn get_decks_by_date(board_id: &str) -> Option<Vec<Deck>>{
  let database = get_database();
  let date = select_date(&database, board_id)?;

  let decks = database
    .get(board_id)?
    .get(&date)?
    .to_vec();

  Some(decks)
}
