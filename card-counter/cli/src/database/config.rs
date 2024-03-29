use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};

use std::env;
use std::fmt;

use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};
use std::str::FromStr;

use super::DatabaseType;
use crate::database::json::config_file;

use crate::{errors::*, kanban::trello::TrelloAuth};

// The possible values that trello accepts for token expiration times
pub static TRELLO_TOKEN_EXPIRATION: &[&str] = &["1hour", "1day", "30days", "never"];

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JiraAuth {
  pub username: String,
  pub api_token: String,
  pub url: String,
}

// impl JiraAuth {
//   fn empty(&self) -> bool {
//     self.username.is_empty() || self.api_token.is_empty() || self.url.is_empty()
//   }
// }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum KanbanBoard {
  Trello(TrelloAuth),
  Jira(JiraAuth),
}

impl fmt::Display for KanbanBoard {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let kanban = match self {
      KanbanBoard::Jira(_) => "Jira",
      KanbanBoard::Trello(_) => "Trello",
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

impl Default for KanbanBoard {
  fn default() -> KanbanBoard {
    KanbanBoard::Trello(TrelloAuth::default())
  }
}

impl FromStr for KanbanBoard {
  type Err = KanbanParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "trello" => Ok(KanbanBoard::Trello(TrelloAuth::default())),
      "jira" => Ok(KanbanBoard::Jira(JiraAuth::default())),
      no_match => Err(KanbanParseError(no_match.to_string())),
    }
  }
}

impl KanbanBoard {
  fn from_env(kanban: &str) -> Option<KanbanBoard> {
    match KanbanBoard::from_str(kanban) {
      Ok(KanbanBoard::Trello(_)) => trello_auth_from_env().ok().map(KanbanBoard::Trello),
      Ok(KanbanBoard::Jira(_)) => jira_auth_from_env().ok().map(KanbanBoard::Jira),
      Err(_) => None,
    }
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
  pub kanban: KanbanBoard,
  // We don't have azure config option because we get aws auth from standard aws sources.
  pub azure: Option<Azure>,
  #[serde(default)]
  pub database: DatabaseType,
  pub database_configuration: Option<DatabaseConfig>,
}

impl Default for Config {
  fn default() -> Config {
    Config {
      kanban: KanbanBoard::default(),
      azure: None,
      database: DatabaseType::default(),
      database_configuration: None,
    }
  }
}

fn database_details(current_config: Option<DatabaseConfig>) -> Option<DatabaseConfig> {
  let _current_config = current_config.unwrap_or_default();
  let database_name = Input::<String>::new()
    .with_prompt("Database Name")
    .default(
      _current_config
        .database_name
        .unwrap_or_else(|| "card-counter".to_string()),
    )
    .interact()
    .ok();

  let container_name = Input::<String>::new()
    .with_prompt("Container Name")
    .default(
      _current_config
        .container_name
        .unwrap_or_else(|| "card-counter".to_string()),
    )
    .interact()
    .ok();

  Some(DatabaseConfig {
    database_name,
    container_name,
  })
}

fn trello_details(kanban: KanbanBoard) -> Result<TrelloAuth> {
  let trello = match kanban {
    KanbanBoard::Jira(_) => TrelloAuth::default(),
    KanbanBoard::Trello(trello) => trello,
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
    .wrap_err_with(|| "There was an error while trying to set token duration.")?;

  let expiration = TRELLO_TOKEN_EXPIRATION[expiration_index].to_string();

  println!("To generate a new Trello API Token please visit go to the link below and paste the token into the prompt:
https://trello.com/1/authorize?expiration={}&name=card-counter&scope=read&response_type=token&key={}", expiration, key);

  let token = Input::<String>::new()
    .with_prompt("Trello API Token")
    .default(trello.token)
    .interact()?;

  Ok(TrelloAuth {
    key,
    token,
    expiration,
  })
}

fn jira_details(kanban: KanbanBoard) -> Result<JiraAuth> {
  let jira = match kanban {
    KanbanBoard::Jira(jira) => jira,
    KanbanBoard::Trello(_) => JiraAuth::default(),
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
    .default(jira.api_token)
    .interact()?;

  Ok(JiraAuth {
    username,
    api_token,
    url,
  })
}

fn kanban_details(kanban: KanbanBoard) -> Result<KanbanBoard> {
  let preferences = [
    KanbanBoard::Trello(TrelloAuth::default()),
    KanbanBoard::Jira(JiraAuth::default()),
  ];
  let choice = Select::new()
    .with_prompt("What kanban board is this for?")
    .items(&preferences)
    .default(0)
    .interact()
    .wrap_err_with(|| "There was an error setting your kanban preference.")?;

  let new_auth = match preferences[choice] {
    KanbanBoard::Trello(_) => KanbanBoard::Trello(trello_details(kanban)?),
    KanbanBoard::Jira(_) => KanbanBoard::Jira(jira_details(kanban)?),
  };

  Ok(new_auth)
}

#[allow(dead_code)]
fn aws_details(aws: Option<AWS>) -> Result<AWS> {
  let _aws = aws.unwrap_or_default();
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
    .default(_aws.region)
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
    .wrap_err_with(|| "There was an error setting database preference.")?;

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
    serde_yaml::from_reader(reader).wrap_err_with(|| "Unable to parse file as YAML")
  }

  // Handles the setup for the app, mostly checking for key and token and giving the proper prompts to the user to get the right info.
  pub fn check_for_auth() -> Result<Option<TrelloAuth>> {
    match (trello_auth_from_env(), Config::from_file()?) {
      (Ok(env), _) => Ok(Some(env)),
      (Err(_), Some(config)) => Ok(config.trello_auth()),
      (Err(e), None) => {
        eprintln!("{}", e);
        Ok(Some(TrelloAuth::default()))
      }
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
    let config = config_file().wrap_err_with(|| "Unable to open config file")?;
    config.set_len(0)?;
    let mut writer = BufWriter::new(config);

    let json = serde_yaml::to_string(&self).wrap_err_with(|| "Unable to parse config")?;

    writer
      .seek(SeekFrom::Start(0))
      .wrap_err_with(|| "Unable to write to file $HOME/.card-counter/card-counter.yaml")?;
    writer
      .write_all(json.as_bytes())
      .wrap_err_with(|| "Unable to write to file $HOME/.card-counter/card-counter.yaml")?;
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

  pub fn init(kanban: Option<&str>) -> Result<Config> {
    let config = Config::from_file_or_default()?;
    if let Some(auth) = KanbanBoard::from_env(kanban.unwrap_or(&config.kanban.to_string())) {
      Ok(Config {
        kanban: auth,
        ..config
      })
    } else {
      Ok(config)
    }
  }

  pub fn trello_auth(self) -> Option<TrelloAuth> {
    if let Ok(auth) = trello_auth_from_env() {
      return Some(auth);
    }
    match self.kanban {
      KanbanBoard::Jira(_) => {
        eprintln!("Unable to get auth details for Trello");
        None
      }
      KanbanBoard::Trello(trello) => Some(trello),
    }
  }

  pub fn jira_auth(self) -> Option<JiraAuth> {
    if let Ok(auth) = jira_auth_from_env() {
      return Some(auth);
    }

    match self.kanban {
      KanbanBoard::Jira(jira) => Some(jira),
      KanbanBoard::Trello(_) => {
        eprintln!("Unable to get auth details for Jira");
        None
      }
    }
  }
}

pub fn trello_auth_from_env() -> Result<TrelloAuth> {
  let key: String = if let Ok(value) = env::var("TRELLO_API_KEY") {
    value
  } else {
    return Err(eyre!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"".to_string()));
  };

  let token: String = if let Ok(value) = env::var("TRELLO_API_TOKEN") {
    value
  } else {
    return Err(eyre!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN".to_string()));
  };

  if key.is_empty() {
    return Err(eyre!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"".to_string()));
  };
  if token.is_empty() {
    return Err(eyre!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN"));
  };
  Ok(TrelloAuth {
    key,
    token,
    expiration: "".to_string(),
  })
}

fn jira_auth_from_env() -> Result<JiraAuth> {
  let username: String = match env::var("JIRA_USERNAME") {
    Ok(value) => value,
    Err(_) => {
      return Err(eyre!("Jira username not found. Please set the environment variable \"JIRA_USERNAME\"
For more information visit https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ ".to_string()));
    }
  };

  let api_token: String = match env::var("JIRA_API_TOKEN") {
    Ok(value) => value,
    Err(_) => {
      return Err(eyre!("Jira API token is missing. Generate a token at https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ and\n and set the token as the environment variable JIRA_API_TOKEN"));
    }
  };

  let url: String = match env::var("JIRA_URL") {
    Ok(value) => value,
    Err(_) => {
      return Err(eyre!("Jira URL is missing. Set the base URL for your Jira account in the environment variable \"JIRA_URL\""));
    }
  };

  if username.is_empty() {
    return Err(eyre!("Jira username not found. Please set the environment variable \"JIRA_USERNAME\"
For more information visit https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ ".to_string()));
  }
  if api_token.is_empty() {
    return Err(eyre!("Jira API token is missing. Generate a token at https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ and\n and set the token as the environment variable JIRA_API_TOKEN"));
  }

  if url.is_empty() {
    return Err(eyre!("Jira URL is missing. Set the base URL for your Jira account in the environment variable \"JIRA_URL\""));
  }

  Ok(JiraAuth {
    username,
    api_token,
    url,
  })
}
