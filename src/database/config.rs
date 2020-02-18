use crate::database::file::{config_file};

use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};
use dialoguer::Input;
use serde::{Serialize, Deserialize};
use crate::errors::*;

trait Default {
  fn defaults() -> Self;
}

// let old_config = get_config();
// let new_config = user_update_prompts(&old_config).unwrap();
// save_config(&new_config).unwrap();


#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
  trello_key: String,
  trello_token: String,
  token_lifetime: Option<String>
}

impl Default for Config {
  fn defaults() -> Config {
    Config {
      trello_token: "".to_string(),
      trello_key: "".to_string(),
      token_lifetime: None
    }
  }
}

pub fn get_config() -> Config {
  let config = config_file().expect("Unable to find config file at $HOME/.card-counter");
  let reader = BufReader::new(&config);

  // We need to know the length of the file or we could erroneously toss a JSON error.
  // We should error out if we can't read metadata.
  if config.metadata().expect("Unable to read metadata for $HOME/.card-counter/config.yaml").len() == 0 {
    return Config::defaults()
  };

  // No Sane default: If we can't parse as json, it might be recoverable and we don't
  // want to overwrite user data
  match serde_yaml::from_reader(reader){
    Ok(db) => db,
    Err(err) => {
      eprintln!("Unable to parse file as YAML");
      panic!("{}", err);
    }
  }
}

pub fn user_update_prompts(config: &Config) -> Result<Config>{
  let trello_key = Input::<String>::new()
    .with_prompt("Trello API Key")
    .default(config.trello_key.clone())
    .interact()?;

  let token_lifetime = None;
  println!("To generate a new Trello API Token please visit go to the link below and paste the token into the prompt:
https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}", trello_key);
  let trello_token = Input::<String>::new()
    .with_prompt("Trello API Token")
    .default(config.trello_token.clone())
    .interact()?;

  Ok(Config{
    trello_key,
    trello_token,
    token_lifetime,
  })
}

pub fn save_config(config: &Config) -> Result<()>{
  let mut writer = BufWriter::new(config_file().chain_err(|| "Unable to open config file")?);

  let json = serde_yaml::to_string(&config).chain_err(|| "Unable to parse config")?;

  writer.seek(SeekFrom::Start(0)).chain_err(|| "Unable to write to file $HOME/.card-counter/config.yaml")?;
  writer.write_all(json.as_bytes()).chain_err(|| "Unable to write to file $HOME/.card-counter/config.yaml")?;
  Ok(())
}
