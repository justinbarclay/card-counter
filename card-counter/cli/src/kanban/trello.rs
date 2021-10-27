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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TrelloAuth {
  pub key: String,
  pub token: String,
  pub expiration: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TrelloList {
  pub id: String,

  #[serde(rename = "idBoard")]
  pub board_id: String,

  pub name: String,

  pub color: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrelloCard {
  pub name: String,

  #[serde(rename = "idList")]
  pub id_list: String,

  #[serde(rename = "idBoard")]
  pub board_id: String,
}

pub struct TrelloClient {
  pub client: reqwest::Client,
  pub auth: TrelloAuth,
}

impl From<TrelloList> for List {
  fn from(list: TrelloList) -> Self {
    List {
      name: list.name,
      id: list.id,
      board_id: list.board_id,
    }
  }
}

impl From<&TrelloList> for List {
  fn from(list: &TrelloList) -> Self {
    List {
      name: list.name.clone(),
      id: list.id.clone(),
      board_id: list.board_id.clone(),
    }
  }
}

impl From<TrelloCard> for Card {
  fn from(card: TrelloCard) -> Self {
    Card {
      name: card.name,
      parent_list: card.id_list,
    }
  }
}

impl From<&TrelloCard> for Card {
  fn from(card: &TrelloCard) -> Self {
    Card {
      name: card.name.clone(),
      parent_list: card.id_list.clone(),
    }
  }
}

impl TrelloClient {
  pub fn init(config: &Config) -> Self {
    match &config.kanban {
      config::KanbanBoard::Trello(auth) => TrelloClient {
        client: reqwest::Client::new(),
        auth: auth.to_owned(),
      },
      _ => panic!("Unable to find information needed to authenticate with Jira API."),
    }
  }
}

// Adds formatting to error message if getting a 401 from the api
pub fn no_authentication(auth: &TrelloAuth, response: &reqwest::Response) -> Result<()> {
  if let Err(err) = response.error_for_status_ref() {
    match err.status() {
      Some(reqwest::StatusCode::UNAUTHORIZED) => {
        return Err(AuthError::Trello(auth.key.clone()).into())
      }
      // Convert private reqwest::error::Error into a trello_error
      _ => return Err(eyre!(err.to_string())),
    }
  };
  Ok(())
}

pub fn trello_to_lists(lists: Vec<TrelloList>) -> Vec<List> {
  lists.iter().map(|list| list.into()).collect()
}

#[async_trait]
impl Kanban for TrelloClient {
  /// Retrieves the name of the board given the id
  async fn get_board(&self, board_id: &str) -> Result<Board> {
    let route = format!(
      "https://api.trello.com/1/boards/{}?key={}&token={}",
      board_id, self.auth.key, self.auth.token
    );

    // Getting all the boards
    let response = self.client.get(&route).send().await?;

    no_authentication(&self.auth, &response)?;

    if let Err(err) = response.error_for_status_ref() {
      match err.status() {
        Some(reqwest::StatusCode::UNAUTHORIZED) => {
          return Err(AuthError::Trello(self.auth.key.clone()).into())
        }
        // Convert private reqwest::error::Error into a trello_error
        _ => return Err(eyre!(err.to_string())),
      }
    };

    Ok(response.json().await?)
  }

  /// Allows the user to select a board from a list
  async fn select_board(&self) -> Result<Board> {
    let route = format!(
      "https://api.trello.com/1/members/me/boards?key={}&token={}",
      self.auth.key, self.auth.token
    );

    // Getting all the boards
    let response = self.client.get(&route).send().await?;

    // TODO: Handle this better
    // maybe create a custom error types for status codes?

    let result: Vec<Board> = response
      .json()
      .await
      .map_err(|_e| JsonParseError("Trello".to_string()))?;

    // Storing it as a hash-map, so we can easily retrieve and return the id
    let boards: HashMap<String, Board> =
      result.iter().fold(HashMap::new(), |mut collection, board| {
        collection.insert(board.name.clone(), board.clone());
        collection
      });

    // Pull out names and get user to select a board name
    let mut board_names: Vec<String> = boards.keys().cloned().collect();
    board_names.sort();
    let name_index: usize = Select::new()
      .with_prompt("Select a board: ")
      .items(&board_names)
      .default(0)
      .max_length(15)
      .interact()
      .wrap_err_with(|| "There was an error while trying to select a board.")?;

    Ok(boards.get(&board_names[name_index]).unwrap().to_owned())
  }

  /// Counts the number of cards for all lists, ignoring lists whose name include the string filter, on a given board.
  async fn get_lists(&self, board_id: &str) -> Result<Vec<List>> {
    let route = format!(
      "https://api.trello.com/1/boards/{}/lists?key={}&token={}",
      board_id, &self.auth.key, &self.auth.token
    );

    let response = self.client.get(&route).send().await?;

    no_authentication(&self.auth, &response)?;

    let lists: Vec<TrelloList> = response
      .json()
      .await
      .map_err(|_e| JsonParseError("Trello".to_string()))?;

    Ok(trello_to_lists(lists))
  }

  /// Returns all cards associated with a board
  async fn get_cards(&self, board_id: &str) -> Result<Vec<Card>> {
    let route = format!(
      "https://api.trello.com/1/boards/{}/cards?card_fields=name&key={}&token={}",
      board_id, self.auth.key, self.auth.token
    );

    let response = self.client.get(&route).send().await?;

    no_authentication(&self.auth, &response)?;

    if let Err(err) = response.error_for_status_ref() {
      match err.status() {
        Some(reqwest::StatusCode::UNAUTHORIZED) => {
          return Err(AuthError::Trello(self.auth.key.clone()).into())
        }
        // Convert private reqwest::error::Error into a trello_error
        _ => return Err(eyre!(err.to_string())),
      }
    };

    let trello_cards: Vec<TrelloCard> = response
      .json()
      .await
      .map_err(|_e| JsonParseError("Trello".to_string()))?;

    Ok(trello_cards.iter().map(|card| card.into()).collect())
  }
}
