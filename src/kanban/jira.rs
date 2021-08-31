use std::collections::HashMap;

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
struct JiraBoard {
  id: u32,
  name: String,
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
  boards: Vec<JiraBoard>,
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

impl From<JiraBoard> for Board {
  fn from(board: JiraBoard) -> Self {
    Board {
      name: board.name,
      id: board.id.to_string(),
    }
  }
}

impl From<&JiraBoard> for Board {
  fn from(board: &JiraBoard) -> Self {
    Board {
      name: board.name.clone(),
      id: board.id.to_string(),
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
      id: column.name.clone(),
      board_id: config.id.to_string(),
    })
    .collect()
}

impl JiraClient {
  pub fn init(config: &Config) -> Self {
    match &config.kanban {
      config::KanbanBoard::Jira(auth) => JiraClient {
        client: reqwest::Client::new(),
        auth: Auth {
          username: auth.username.clone(),
          base_url: auth.url.clone(),
          token: auth.api_token.clone(),
        },
      },
      _ => panic!("Unable to find information needed to authenticate with Jira API."),
    }
  }
}

#[async_trait]
impl Kanban for JiraClient {
  async fn get_board(&self, board_id: &str) -> Result<Board> {
    let route = format!("{}/rest/agile/1.0/board/{}", self.auth.base_url, board_id);
    let board: JiraBoard = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await?
      .json()
      .await?;

    Ok(board.into())
  }

  async fn select_board(&self) -> Result<Board> {
    let route = format!("{}/rest/agile/1.0/board", self.auth.base_url);

    let response = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await?;

    let result: PagedBoards = response.json().await?;

    // Storing it as a hash-map, so we can easily retrieve and return the id
    let boards: _ = result.boards.iter().fold(
      HashMap::new(),
      |mut collection: HashMap<String, Board>, board: &JiraBoard| {
        collection.insert(board.name.clone(), board.into());
        collection
      },
    );

    // Pull out names and get user to select a board name
    let mut board_names: Vec<String> = boards.keys().cloned().collect();
    board_names.sort();
    let name_index: usize = Select::new()
      .with_prompt("Select a board: ")
      .items(&board_names)
      .default(0)
      .paged(true)
      .interact()
      .wrap_err_with(|| "There was an error while trying to select a board.")?;

    Ok(boards.get(&board_names[name_index])?.to_owned())
  }

  async fn get_lists(&self, board_id: &str) -> Result<Vec<List>> {
    let route = format!(
      "{}/rest/agile/1.0/board/{}/configuration",
      self.auth.base_url, board_id
    );
    let config: Configuration = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await?
      .json()
      .await?;

    Ok(config.into())
  }

  async fn get_cards(&self, board_id: &str) -> Result<Vec<Card>> {
    let route = format!(
      "{}/rest/agile/1.0/board/{}/issue",
      self.auth.base_url, board_id
    );
    let response: Issues = self
      .client
      .get(&route)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await?
      .json()
      .await?;

    Ok(response.issues.iter().map(|issue| issue.into()).collect())
  }
}
