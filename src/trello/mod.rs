use crate::errors::*;
use dialoguer::Select;
/// Structures for serializing and de-serializing responses from Trello
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

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

  #[serde(rename = "idList")]
  pub id_list: String,
}

pub fn auth_from_env() -> Option<Auth> {
  let key: String = match env::var("TRELLO_API_KEY") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Trello API key not found. Please visit https://trel lo.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
      return None;
    }
  };

  let token: String = match env::var("TRELLO_API_TOKEN") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN", key);
      return None;
    }
  };

  if key.is_empty() {
    eprintln!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
    return None;
  }
  if token.is_empty() {
    eprintln!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN", key);
    return None;
  }
  Some(Auth { key, token })
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

/// Allows the user to select a board from a list
pub async fn select_board(auth: &Auth) -> Result<Board> {
  let client = reqwest::Client::new();

  // Getting all the boards
  let response = client
    .get(&format!(
      "https://api.trello.com/1/members/me/boards?key={}&token={}",
      auth.key, auth.token
    ))
    .send()
    .await?;

  // TODO: Handle this better
  // maybe create a custom error types for status codes?

  let result: Vec<Board> = response.json().await?;

  // Storing it as a hash-map, so we can easily retrieve and return the id
  let boards: HashMap<String, Board> =
    result.iter().fold(HashMap::new(), |mut collection, board| {
      collection.insert(board.name.clone(), board.clone());
      collection
    });

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

pub fn collect_cards(cards: Vec<Card>) -> HashMap<String, Vec<Card>> {
  cards.into_iter().fold(
    HashMap::new(),
    |mut collection: HashMap<String, Vec<Card>>, card: Card| {
      let list_id = card.id_list.clone();
      collection.entry(list_id).or_default().push(card);
      collection
    },
  )
}

/// Returns all cards associated with a board
pub async fn get_cards(auth: &Auth, board_id: &str) -> Result<Vec<Card>> {
  let client = reqwest::Client::new();
  let response = client
    .get(&format!(
      "https://api.trello.com/1/boards/{}/cards?card_fields=name&key={}&token={}",
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

  let cards: Vec<Card> = response
    .json()
    .await
    .chain_err(|| "There was a problem parsing JSON.")?;

  Ok(cards)
}
