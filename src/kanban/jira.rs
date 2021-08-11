use std::{collections::HashMap, env};

use crate::{
  database::config,
  database::config::Config,
  errors::*,
  kanban::{Board, Card, Kanban, List},
};

use async_trait::async_trait;

use dialoguer::Select;
use reqwest;
use serde::{Deserialize, Serialize};

struct Auth {
  username: String,
  token: String,
  base_url: String,
}
// Jesus, the amount of structures we have to define
// to get some simple kanban stats from Jira is incredible
#[derive(Serialize, Deserialize, Debug)]
struct Pagination {
  #[serde(rename = "startAt")]
  start_at: u32,

  #[serde(rename = "maxResults")]
  max_results: u32,
  total: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Status {
  id: String,
  name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct IssueFields {
  summary: String,
  status: Status,
}

#[derive(Serialize, Deserialize, Debug)]
struct Issue {
  id: String,
  fields: IssueFields,
}

#[derive(Serialize, Deserialize, Debug)]
struct PagedBoards {
  #[serde(flatten)]
  pagination: Pagination,
  #[serde(rename = "values")]
  boards: Vec<Board>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Issues {
  #[serde(flatten)]
  pagination: Pagination,
  issues: Vec<Issue>,
}

pub struct JiraClient {
  client: reqwest::Client,
  auth: Auth,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Column {
  name: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ColumnConfig {
  columns: Vec<Column>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Configuration {
  id: u32,
  #[serde(rename = "name")]
  board_name: String,
  #[serde(rename = "columnConfig")]
  column_config: ColumnConfig,
}

impl From<Issue> for Card {
  fn from(issue: Issue) -> Self {
    Card {
      name: issue.fields.summary,
      parent_list: issue.fields.status.name,
    }
  }
}

impl From<&Issue> for Card {
  fn from(issue: &Issue) -> Self {
    Card {
      name: issue.fields.summary.clone(),
      parent_list: issue.fields.status.name.clone(),
    }
  }
}

impl From<Configuration> for Vec<List> {
  fn from(config: Configuration) -> Self {
    config_to_lists(&config)
  }
}
impl From<&Configuration> for Vec<List> {
  fn from(config: &Configuration) -> Self {
    config_to_lists(config)
  }
}

pub fn config_to_lists(config: &Configuration) -> Vec<List> {
  config
    .column_config
    .columns
    .iter()
    .map(|column| List {
      name: column.name.clone(),
      board_id: config.id,
    })
    .collect()
}

impl JiraClient {
  pub fn init(config: &Config) -> Self {
    match (&config.kanban, auth_from_env()) {
      (config::Board::Jira(auth), _) => {
        return JiraClient {
          client: reqwest::Client::new(),
          auth: Auth {
            username: auth.username.clone(),
            base_url: auth.url.clone(),
            token: auth.api_token.clone(),
          },
        }
      }
      (_, Some(auth)) => JiraClient {
        client: reqwest::Client::new(),
        auth,
      },
      (_, _) => panic!("Unable to find information needed to authenticate with Jira API."),
    }
  }
}

#[async_trait]
impl Kanban for JiraClient {
  async fn get_board(&self, board_id: u32) -> Result<Board> {
    let route = format!("{}/rest/agile/1.0/board/{}", self.auth.base_url, board_id);
    Ok(
      self
        .client
        .get(&route)
        .basic_auth(&self.auth.username, Some(&self.auth.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap(),
    )
  }
  async fn get_lists(&self, board_id: u32) -> Result<Vec<List>> {
    let route = format!(
      "{}/rest/agile/1.0/board/{}/configuration",
      self.auth.base_url, board_id
    );
    let config: Configuration = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .unwrap()
      .json()
      .await
      .unwrap();

    Ok(config.into())
  }

  async fn get_cards(&self, board_id: u32) -> Result<Vec<Card>> {
    let route = format!(
      "{}/rest/agile/1.0/board/{}/issue",
      self.auth.base_url, board_id
    );
    let response: Issues = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .unwrap()
      .json()
      .await
      .unwrap();

    Ok(response.issues.iter().map(|issue| issue.into()).collect())
  }

  async fn select_board(&self) -> Result<Board> {
    let route = format!("{}/rest/agile/1.0/board", self.auth.base_url);

    let response = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .unwrap();
    let result: PagedBoards = response.json().await?;

    // Storing it as a hash-map, so we can easily retrieve and return the id
    let boards: _ = result.boards.iter().fold(
      HashMap::new(),
      |mut collection: HashMap<String, Board>, board: &Board| {
        collection.insert(board.name.clone(), board.to_owned());
        collection
      },
    );

    // Pull out names and get user to select a board name
    let mut board_names: Vec<String> = boards.keys().map(|key: &String| key.clone()).collect();
    board_names.sort();
    let name_index: usize = Select::new()
      .with_prompt("Select a board: ")
      .items(&board_names)
      .default(0)
      .paged(true)
      .interact()
      .chain_err(|| "There was an error while trying to select a board.")?;

    Ok(boards.get(&board_names[name_index]).unwrap().to_owned())
  }
}

fn auth_from_env() -> Option<Auth> {
  let username: String = match env::var("JIRA_USERNAME") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Jira username not found. Please set the environment variable \"JIRA_USERNAME\"");
      eprintln!("For more information visit https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ for more information");
      return None;
    }
  };

  let token: String = match env::var("JIRA_API_TOKEN") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Jira API token is missing. Generate a token at https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ and\n and set the token as the environment variable JIRA_API_TOKEN");
      return None;
    }
  };

  let base_url: String = match env::var("JIRA_URL") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Jira URL is missing. Set the base URL for your Jira account in the environment variable \"JIRA_URL\"");
      eprintln!("For more information visit https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ for more information");
      return None;
    }
  };

  if username.is_empty() {
    eprintln!("Jira username not found. Please set the environment variable \"JIRA_USERNAME\"");
    eprintln!("For more information visitvisit https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ for more info. and");
    return None;
  }
  if token.is_empty() {
    eprintln!("Jira API token is missing. Generate a token at https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ and\n and set the token as the environment variable JIRA_API_TOKEN");
    return None;
  }

  if base_url.is_empty() {
    eprintln!("Jira URL is missing. Set the base URL for your Jira account in the environment variable \"JIRA_URL\"");
    eprintln!("For more information visit https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/ for more information");
    return None;
  }
  Some(Auth {
    username,
    token,
    base_url,
  })
}
