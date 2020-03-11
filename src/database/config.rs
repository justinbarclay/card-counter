use crate::database::file::{config_file};

use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};
use dialoguer::{Input, Select};
use serde::{Serialize, Deserialize};
use crate::trello::Auth;
use crate::errors::*;

trait Default {
  fn defaults() -> Self;
}

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

// The possible values that trello accepts for token expiration times
pub static TOKEN_EXPIRATION: &'static [&str] = &["1hour", "1day", "30days", "never"];

// This is a little bit messy, we
pub fn get_config() -> Result<Option<Config>> {
  let config = match config_file(){
    Ok(file) => file,
    Err(_) => return Ok(None)
  };

  let reader = BufReader::new(&config);

  // We need to know the length of the file or we could erroneously toss a JSON error.
  // We should error out if we can't read metadata.
  if config.metadata().expect("Unable to read metadata for $HOME/.card-counter/config.yaml").len() == 0 {
    return Ok(None)
  };

  // No Sane default: If we can't parse as json, it might be recoverable and we don't
  // want to overwrite user data
  serde_yaml::from_reader(reader).chain_err(|| "Unable to parse file as YAML")
}

pub fn user_update_prompts(config: &Config) -> Result<Config>{
  let trello_key = Input::<String>::new()
    .with_prompt("Trello API Key")
    .default(config.trello_key.clone())
    .interact()?;

  let expiration_index: usize = Select::new()
    .with_prompt("How long until your tokens expires?")
    .items(TOKEN_EXPIRATION)
    .default(0)
    .interact()
    .chain_err(|| "There was an error while trying to set token duration.")?;

  let expiration_time = TOKEN_EXPIRATION[expiration_index];

  println!("To generate a new Trello API Token please visit go to the link below and paste the token into the prompt:
https://trello.com/1/authorize?expiration={}&name=card-counter&scope=read&response_type=token&key={}", expiration_time, trello_key);
  let trello_token = Input::<String>::new()
    .with_prompt("Trello API Token")
    .default(config.trello_token.clone())
    .interact()?;

  Ok(Config{
    trello_key,
    trello_token,
    token_lifetime: Some(expiration_time.to_string()),
  })
}

pub fn save_config(config: &Config) -> Result<()>{
  let mut writer = BufWriter::new(config_file().chain_err(|| "Unable to open config file")?);

  let json = serde_yaml::to_string(&config).chain_err(|| "Unable to parse config")?;

  writer.seek(SeekFrom::Start(0)).chain_err(|| "Unable to write to file $HOME/.card-counter/config.yaml")?;
  writer.write_all(json.as_bytes()).chain_err(|| "Unable to write to file $HOME/.card-counter/config.yaml")?;
  Ok(())
}

pub fn update_config() -> Result<()>{
  let config = match get_config()?{
    Some(config) => config,
    None => Config::defaults()
  };
  let new_config = user_update_prompts(&config)?;
  save_config(&new_config).unwrap();
  Ok(())
}

//
pub fn auth_from_config() -> Result<Option<Auth>>{
  let config = match get_config()?{
    Some(config) => config,
    None => return Ok(None)
  };

  Ok(Some(
    Auth{
      key: config.trello_key,
      token: config.trello_token
    }))
}
