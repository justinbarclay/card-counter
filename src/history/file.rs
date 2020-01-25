use std::fs;
use std::fs::File;
use std::path::PathBuf;
use dirs::home_dir;
use std::io::{Error, ErrorKind};

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

  if path.exists(){
    File::open(path)
  } else {
    File::create(path)
  }
}

/// Opens and returns file handle for the config file. If no file is found it creates ones.
pub fn config() -> std::io::Result<File>{
  get_file(CONFIG)
}

/// Opens and returns the file handle for the history file. If no file is found it creates ones.
pub fn history() -> std::io::Result<File>{
  get_file(HISTORY)
}
