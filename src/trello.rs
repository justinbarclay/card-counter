/// Structures for serializing and de-serializing responses from Trello
use serde::{Deserialize, Serialize};

use crate::errors::*;
// Unofficial struct to hold the key and token for working with the trello api
#[derive(Clone, Debug)]
pub struct Auth {
  pub key: String,
  pub token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Board {
  pub id: String,

  pub name: String,

  pub desc: String,

  pub closed: Option<bool>,

  #[serde(rename = "idOrganization")]
  pub id_organization: Option<String>,

  pub pinned: Option<bool>,

  pub url: String,

  #[serde(rename = "shortUrl")]
  pub short_url: String,

  pub starred: Option<bool>,

  #[serde(rename = "enterpriseOwned")]
  pub enterprise_owned: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct List {
  pub id: String,

  #[serde(rename = "idBoard")]
  pub id_board: String,

  pub name: String,

  pub color: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Card {
  pub name: String,
}
// Adds formatting to error message if getting a 401 from the api
pub fn no_authentication(auth: &Auth, response: &reqwest::Response) -> Result<()> {
  if let Err(err) = response.error_for_status_ref() {
    match err.status() {
      Some(reqwest::StatusCode::UNAUTHORIZED) => {
        return Err(ErrorKind::InvalidAuthInformation(auth.clone()).into())
      }
      // Convert private reqwest::error::Error into a trello_error
      _ => return Err(err.to_string().into()),
    }
  };
  Ok(())
}

/// Counts the number of cards for all lists, ignoring lists whose name include the string filter, on a given board.
pub async fn get_lists(auth: &Auth, board_id: &str) -> Result<Vec<List>> {
  let client = reqwest::Client::new();
  let response = client
    .get(&format!(
      "https://api.trello.com/1/boards/{}/lists?key={}&token={}",
      board_id, auth.key, auth.token
    ))
    .send()
    .await?;

  no_authentication(auth, &response)?;

  let lists: Vec<List> = response.json().await?;

  Ok(lists)
}

/// Retrieves the name of the board given the id
pub async fn get_board(board_id: &str, auth: &Auth) -> Result<Board> {
  let client = reqwest::Client::new();

  // Getting all the boards
  let response = client
    .get(&format!(
      "https://api.trello.com/1/boards/{}?key={}&token={}",
      board_id, auth.key, auth.token
    ))
    .send()
    .await?;

  no_authentication(auth, &response)?;

  if let Err(err) = response.error_for_status_ref() {
    match err.status() {
      Some(reqwest::StatusCode::UNAUTHORIZED) => {
        return Err(ErrorKind::InvalidAuthInformation(auth.clone()).into())
      }
      // Convert private reqwest::error::Error into a trello_error
      _ => return Err(err.to_string().into()),
    }
  };

  let board: Board = response.json().await?;
  Ok(board)
}

pub async fn get_cards(auth: &Auth, list_id: &str) -> Result<Vec<Card>> {
  let client = reqwest::Client::new();
  let response = client
    .get(&format!(
      "https://api.trello.com/1/lists/{}/cards?card_fields=name&key={}&token={}",
      list_id, auth.key, auth.token
    ))
    .send()
    .await?;

  no_authentication(auth, &response)?;

  if let Err(err) = response.error_for_status_ref() {
    match err.status() {
      Some(reqwest::StatusCode::UNAUTHORIZED) => {
        return Err(ErrorKind::InvalidAuthInformation(auth.clone()).into())
      }
      // Convert private reqwest::error::Error into a trello_error
      _ => return Err(err.to_string().into()),
    }
  };

  let cards: Vec<Card> = response
    .json()
    .await
    .chain_err(|| "There was a problem parsing JSON.")?;

  Ok(cards)
}
