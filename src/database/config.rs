use std::fmt;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};

use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};

use super::DatabaseType;
use crate::database::json::config_file;
use crate::{
  errors::*,
  trello::{self, Auth},
};

// The possible values that trello accepts for token expiration times
pub static TRELLO_TOKEN_EXPIRATION: &'static [&str] = &["1hour", "1day", "30days", "never"];

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TrelloAuth {
  key: String,
  token: String,
  expiration: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JiraAuth {
  pub username: String,
  pub api_token: String,
  pub url: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Board {
  Trello(TrelloAuth),
  Jira(JiraAuth),
}

impl fmt::Display for Board {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let kanban = match self {
      &Board::Jira(_) => "Jira",
      &Board::Trello(_) => "Trello",
    };
    write!(f, "{}", kanban)
  }
}

impl Default for TrelloAuth {
  fn default() -> TrelloAuth {
    TrelloAuth {
      token: "".to_string(),
      key: "".to_string(),
      expiration: "1day".to_string(),
    }
  }
}
impl Default for JiraAuth {
  fn default() -> JiraAuth {
    JiraAuth {
      username: "".to_string(),
      api_token: "".to_string(),
      url: "".to_string(),
    }
  }
}

impl Default for Board {
  fn default() -> Board {
    Board::Trello(TrelloAuth::default())
  }
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct AWS {
  secret_access_key: String,
  access_key_id: String,
  region: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]

pub struct Azure {
  cosmos_master_key: String,
  cosmos_account: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct DatabaseConfig {
  pub database_name: Option<String>,
  pub container_name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
  pub kanban: Board,
  // We don't have azure config option because we get aws auth from standard aws sources.
  pub azure: Option<Azure>,
  #[serde(default)]
  pub database: DatabaseType,
  pub database_configuration: Option<DatabaseConfig>,
}

impl Default for Config {
  fn default() -> Config {
    Config {
      kanban: Board::default(),
      azure: None,
      database: DatabaseType::default(),
      database_configuration: None,
    }
  }
}

fn database_details(current_config: Option<DatabaseConfig>) -> Option<DatabaseConfig> {
  let _current_config = current_config.unwrap_or(Default::default());
  let database_name = Input::<String>::new()
    .with_prompt("Database Name")
    .default(
      _current_config
        .database_name
        .unwrap_or("card-counter".to_string())
        .clone(),
    )
    .interact()
    .ok();

  let container_name = Input::<String>::new()
    .with_prompt("Container Name")
    .default(
      _current_config
        .container_name
        .unwrap_or("card-counter".to_string())
        .clone(),
    )
    .interact()
    .ok();

  Some(DatabaseConfig {
    database_name,
    container_name,
  })
}

fn trello_details(kanban: Board) -> Result<TrelloAuth> {
  let trello = match kanban {
    Board::Jira(_) => TrelloAuth::default(),
    Board::Trello(trello) => trello,
  };

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

  Ok(TrelloAuth {
    key,
    token,
    expiration,
  })
}

fn jira_details(kanban: Board) -> Result<JiraAuth> {
  let jira = match kanban {
    Board::Jira(jira) => jira,
    Board::Trello(_) => JiraAuth::default(),
  };

  let url = Input::<String>::new()
    .with_prompt("Jira URL:")
    .default(jira.url.clone())
    .interact()?;

  let username = Input::<String>::new()
    .with_prompt("Jira Username:")
    .default(jira.username.clone())
    .interact()?;

  println!(
    "To generate an API token for your Jira account please follow the instructions here at:
https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account"
  );

  let api_token = Input::<String>::new()
    .with_prompt("Jira API Token")
    .default(jira.api_token.clone())
    .interact()?;

  Ok(JiraAuth {
    username,
    api_token,
    url,
  })
}

fn kanban_details(kanban: Board) -> Result<Board> {
  let preferences = [
    Board::Trello(TrelloAuth::default()),
    Board::Jira(JiraAuth::default()),
  ];
  let choice = Select::new()
    .with_prompt("What kanban board is this for?")
    .items(&preferences)
    .default(0)
    .interact()
    .chain_err(|| "There was an error setting your kanban preference.")?;

  let new_auth = match preferences[choice] {
    Board::Trello(_) => Board::Trello(trello_details(kanban)?),
    Board::Jira(_) => Board::Jira(jira_details(kanban)?),
  };

  Ok(new_auth)
}

#[allow(dead_code)]
fn aws_details(aws: Option<AWS>) -> Result<AWS> {
  let _aws = aws.unwrap_or(Default::default());
  let access_key_id = Input::<String>::new()
    .with_prompt("Access Key ID")
    .default(_aws.access_key_id.clone())
    .interact()?;

  let secret_access_key = Input::<String>::new()
    .with_prompt("Secret Access Key")
    .default(_aws.secret_access_key.clone())
    .interact()?;

  let region = Input::<String>::new()
    .with_prompt("Region")
    .default(_aws.region.clone())
    .interact()?;

  Ok(AWS {
    access_key_id,
    secret_access_key,
    region,
  })
}

fn database_preference() -> Result<DatabaseType> {
  let preferences = [
    DatabaseType::Local,
    DatabaseType::Aws,   /*, DatabaseType::Azure */
    DatabaseType::Azure, /*, DatabaseType::Azure */
  ];
  let index = Select::new()
    .with_prompt("What database would you prefer?")
    .items(&preferences)
    .default(0)
    .interact()
    .chain_err(|| "There was an error setting database preference.")?;

  Ok(preferences[index].clone())
}

impl Config {
  pub fn from_file() -> Result<Option<Config>> {
    let config = match config_file() {
      Ok(file) => file,
      Err(_) => return Ok(None),
    };

    let reader = BufReader::new(&config);

    // We need to know the length of the file or we could erroneously toss a JSON error.
    // We should error out if we can't read metadata.
    if config
      .metadata()
      .expect("Unable to read metadata for $HOME/.card-counter/config.yaml")
      .len()
      == 0
    {
      return Ok(None);
    };

    // No Sane default: If we can't parse as json, it might be recoverable and we don't
    // want to overwrite user data
    serde_yaml::from_reader(reader).chain_err(|| "Unable to parse file as YAML")
  }

  // Handles the setup for the app, mostly checking for key and token and giving the proper prompts to the user to get the right info.
  pub fn check_for_auth() -> Result<Option<Auth>> {
    match Config::from_file()? {
      Some(config) => Ok(Some(config.trello_auth())),
      None => Ok(trello::auth_from_env()),
    }
  }

  pub fn user_update_prompts(mut self) -> Result<Config> {
    self.kanban = kanban_details(self.kanban)?;
    self.database = database_preference()?;
    if self.database == DatabaseType::Azure {
      println!("What are your Cosmos database and container names?");
      self.database_configuration = database_details(self.database_configuration);
    }
    Ok(self)
  }

  pub fn persist(self) -> Result<()> {
    let config = config_file().chain_err(|| "Unable to open config file")?;
    config.set_len(0)?;
    let mut writer = BufWriter::new(config);

    let json = serde_yaml::to_string(&self).chain_err(|| "Unable to parse config")?;

    writer
      .seek(SeekFrom::Start(0))
      .chain_err(|| "Unable to write to file $HOME/.card-counter/card-counter.yaml")?;
    writer
      .write_all(json.as_bytes())
      .chain_err(|| "Unable to write to file $HOME/.card-counter/card-counter.yaml")?;
    Ok(())
  }

  pub fn update_file(self) -> Result<()> {
    self.user_update_prompts()?.persist().unwrap();
    Ok(())
  }

  pub fn from_file_or_default() -> Result<Config> {
    match Config::from_file()? {
      Some(config) => Ok(config),
      None => Ok(Config::default()),
    }
  }

  pub fn trello_auth(self) -> Auth {
    match self.kanban {
      Board::Jira(_) => panic!("Unable to get auth details for Jira"),
      Board::Trello(trello) => Auth {
        key: trello.key,
        token: trello.token,
      },
    }
  }
}
