use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};

use dialoguer::{Input, Select};
use serde::{Serialize, Deserialize};

use crate::trello::Auth;
use crate::errors::*;
use crate::database::file::{config_file};

trait Default {
  fn default() -> Self;
}

// The possible values that trello accepts for token expiration times
pub static TRELLO_TOKEN_EXPIRATION: &'static [&str] = &["1hour", "1day", "30days", "never"];

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Trello{
  key: String,
  token: String,
  expiration: String
}

impl Default for Trello {
  fn default() -> Trello {
    Trello {
      token: "".to_string(),
      key: "".to_string(),
      expiration: "1day".to_string()
    }
  }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct AWS {
  secret_access_key: String,
  access_key_id: String
}

impl Default for AWS {
  fn default() -> AWS {
    AWS {
      secret_access_key: "".to_string(),
      access_key_id: "".to_string(),
    }
  }
}


#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
  trello: Trello,
  aws: Option<AWS>
}

impl Default for Config {
  fn default() -> Config {
    Config {
      trello: Trello::default(),
      aws: Some(AWS::default())
    }
  }
}

fn trello_details(trello: &Trello) -> Result<Trello>{
    let key = Input::<String>::new()
    .with_prompt("Trello API Key")
    .default(trello.key.clone())
    .interact()?;

  let expiration_index: usize = Select::new()
    .with_prompt("How long until your tokens expires?")
    .items(TRELLO_TOKEN_EXPIRATION)
    .default(0)
    .interact()
    .chain_err(|| "There was an error while trying to set token duration.")?;

  let expiration = TRELLO_TOKEN_EXPIRATION[expiration_index].to_string();

  println!("To generate a new Trello API Token please visit go to the link below and paste the token into the prompt:
https://trello.com/1/authorize?expiration={}&name=card-counter&scope=read&response_type=token&key={}", expiration, key);

  let token = Input::<String>::new()
    .with_prompt("Trello API Token")
    .default(trello.token.clone())
    .interact()?;

  Ok(Trello {
    key,
    token,
    expiration: expiration
  })
}

fn aws_details(aws: &AWS) -> Result<AWS>{
    let access_key_id = Input::<String>::new()
    .with_prompt("Access Key ID")
    .default(aws.access_key_id.clone())
    .interact()?;

  let secret_access_key = Input::<String>::new()
    .with_prompt("Secret Access Key")
    .default(aws.secret_access_key.clone())
    .interact()?;

  Ok( AWS {
    access_key_id,
    secret_access_key
  })
}

impl Config {
  pub fn from_file() -> Result<Option<Config>> {
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

  pub fn user_update_prompts(mut self) -> Result<Config>{
    let trello = trello_details(&self.trello)?;
    self.trello = trello;
    Ok(self)
  }

  pub fn persist (self) -> Result<()>{
    let mut writer = BufWriter::new(config_file().chain_err(|| "Unable to open config file")?);

    let json = serde_yaml::to_string(&self).chain_err(|| "Unable to parse config")?;

    writer.seek(SeekFrom::Start(0)).chain_err(|| "Unable to write to file $HOME/.card-counter/config.yaml")?;
    writer.write_all(json.as_bytes()).chain_err(|| "Unable to write to file $HOME/.card-counter/config.yaml")?;
    Ok(())
  }

  pub fn update_file(self) -> Result<()>{
    self.user_update_prompts()?
      .persist()
      .unwrap();
    Ok(())
  }

  pub fn from_file_or_default() -> Result<Config>{
    match Config::from_file()? {
      Some(config) => Ok(config),
      None => Ok(Config::default())
    }
  }

  pub fn trello_auth(self) -> Auth{
    Auth{
      key: self.trello.key,
      token: self.trello.token
    }
  }

  pub fn aws_auth(self) -> Auth{
    match self.aws {
      Some(aws) => Auth{
        key: aws.secret_access_key,
        token: aws.access_key_id
      },
      None => Auth {
        key: "".to_string(),
        token: "".to_string()
      }
    }
  }
}
