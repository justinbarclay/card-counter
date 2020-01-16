// File for retrieving cards from trello and scoring them
use std::collections::HashMap;

use dialoguer::Select;

use regex::Regex;

use crate::trello::{Board, Card, Auth, List};

/// A deck represents some summary data about a list of Trello cards
pub struct Deck{
  // Is the name of the list that the Deck represents
  pub name: String,
  // Represents total numbers of cards in the list
  pub size: usize,
  // Represents the cumulative total score of all the cards in the lists
  pub score: i32,
  // Represents the total amount of unscored cards in the list
  pub unscored: i32

}

/// Allows the user to select a board from a list
pub async fn get_board_id(auth: Auth) -> Result<String, Box<dyn std::error::Error>> {
  let client = reqwest::Client::new();

  // Getting all the boards
  let result: Vec<Board> = client.get(&format!("https://api.trello.com/1/members/me/boards?key={}&token={}", auth.key, auth.token))
    .send()
    .await?
    .json()
    .await?;


  // Storing it as a hash-map, so we can easily retrieve and return the id
  let boards: HashMap<String, String> = result.iter().fold(HashMap::new(), |mut collection, board| {
    collection.insert(board.name.clone(), board.id.clone());
    collection
  });

  // Pull out names and get user to select a board name
  let board_names: Vec<String> = boards.keys().map(|key: &String| key.clone()).collect();
  let name_index: usize = Select::new()
    .with_prompt("Select a board: ")
    .items(&board_names)
    .default(0)
    .interact()?;

  Ok(boards
     .get(&board_names[name_index])
     .unwrap()
     .to_string())
}

/// Counts the number of cards for all lists, ignoring lists whose name include the string filter, on a given board.
pub async fn get_lists(auth: Auth, board_id: &str, filter: Option<&str>) -> Result< Vec<List>, Box<dyn std::error::Error>>{
  let client = reqwest::Client::new();
  let lists: Vec<List> = client.get(&format!("https://api.trello.com/1/boards/{}/lists?key={}&token={}", board_id, auth.key, auth.token))
    .send()
    .await?
    .json()
    .await?;

  Ok(lists.iter().fold(Vec::new(), |mut container, list| {
    match filter {
      Some(value) => {
        if !list.name.contains(value) {
          container.push(list.clone());
        }
      },
      None => container.push(list.clone())
    };

    container
  }))
}


/// Iterates over all the cards in each lists and builds up the stats for a deck of cards
pub async fn build_decks(auth: Auth, lists: Vec<List>) ->  Result< Vec<Deck>, Box<dyn std::error::Error>>{
  let client = reqwest::Client::new();
  let mut decks = Vec::new();
  for list in lists {
    let cards: Vec<Card> = client
      .get(&format!("https://api.trello.com/1/lists/{}/cards?card_fields=name&key={}&token={}", list.id, auth.key, auth.token))
      .send()
      .await?
    .json()
      .await?;

    decks.push(
      Deck{
        name: list.name,
        size: cards.len(),
        score: cards.iter().fold(0, |total, card|{
          match get_score(&card.name){
            Some(score) => total + score,
            None => total
          }

        }),
        unscored: cards.iter().fold(0, |total, card| {
          match get_score(&card.name){
            Some(_) => total,
            None => total + 1
          }
        })
      });
  }

  Ok(decks)
}

/// Extracts a score from a trello card, based on using [] or (). If no score is found a 0 is returned
fn get_score(maybe_points: &str) -> Option<i32>{
  // this will capture on "(0)" or "[0]" where 0 is an arbitrary sized digit
  let re = Regex::new(r"\[(\d+)\]|\((\d+)\)").unwrap();
  let cap = match re.captures(&maybe_points) {
    Some(cap) => cap,
    // Early exit
    None => return None
  };

  match cap.get(0) {
    Some(capture) =>{
      let a_match = String::from(capture.as_str());
      // We need to strip the brackets
      let maybe_number = &a_match[1..a_match.len()-1];
      match maybe_number.parse::<i32>() {
        Ok(number) => Some(number),
        Err(_) => None
      }
    },
    None => None
  }
}

pub mod test{
  use super::get_score;

  #[test]
  fn get_score_handles_curlies(){
    assert_eq!(get_score("(10)"), Some(10));

    assert_eq!(get_score("()"), None);

    assert_eq!(get_score("(z)"), None);
    assert_eq!(get_score("(10z)"), None);
  }

  #[test]
  fn get_score_handles_angles(){
    assert_eq!(get_score("[10]"), Some(10));

    assert_eq!(get_score("[]"), None);

    assert_eq!(get_score("[z]"), None);
    assert_eq!(get_score("[10z]"), None);
  }

  #[test]
  fn get_score_handles_curlies_and_angles(){
    assert_eq!(get_score("[10](9)"), Some(10));

    assert_eq!(get_score("[]()"), None);

    assert_eq!(get_score("[z](9)"), Some(9));
    assert_eq!(get_score("[9](z)"), Some(9));
    assert_eq!(get_score("[](9)"), Some(9));
    assert_eq!(get_score("[9]()"), Some(9));

    // Square brackets should be prioritized over round brackets
    // assert_eq!(get_score("(9)[10]"), Some(10));

    assert_eq!(get_score("[9z]()"), None);
  }

  #[test]
  fn get_score_handles_arbitrarily_sized_digits(){
    assert_eq!(get_score("[100000000](9)"), Some(100000000));
  }
}
