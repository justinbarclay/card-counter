// File for retrieving cards from trello and scoring them
use std::collections::HashMap;

use dialoguer::Select;
use prettytable::Table;
use regex::Regex;
use regex::Captures;
use serde::{Serialize, Deserialize};
use std::convert::TryInto;
use chrono::NaiveDateTime;
use crate::trello::{Board, Card, Auth, List};
use crate::database::file::{get_database, get_latest_entry};
/// A deck represents some summary data about a list of Trello cards
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Deck{
  // Is the name of the list that the Deck represents
  pub name: String,
  // Represents total numbers of cards in the list
  pub size: usize,
  // Represents the cumulative total effort of all the cards in the list
  pub score: i32,
  // Represents the total amount of unscored cards in the list
  pub unscored: i32,
  // Represents the estimated effort for all cards in the list during the sprint
  pub estimated: i32
}

/// A score is a result of a user estimating the effort required for a card `()` and then optionally
/// a correction `[]` after they've completed the card and found out it was worth more or less effort.
#[derive(PartialEq, Debug)]
pub struct Score{
  estimated: Option<i32>,
  correction: Option<i32>
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
  let mut board_names: Vec<String> = boards.keys().map(|key: &String| key.clone()).collect();
  board_names.sort();
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

    let (score, unscored, estimated) = cards.iter().fold((0,0,0), |(total, unscored, estimate), card|{
      match get_score(&card.name){
        Some(score) => {
          if let Some(correction) = score.correction {
            (total + correction, unscored, estimate)
          } else {
            (total + score.estimated.unwrap(), unscored, estimate + score.estimated.unwrap())
          }
        },
        None => (total, unscored + 1, estimate)
      }
    });

    decks.push(
      Deck{
        name: list.name,
        size: cards.len(),
        score,
        unscored,
        estimated,
      });
  }

  Ok(decks)
}

/// Converts a trello effort score either [\d] or (\d) into a number.
/// If the item inside the brackets can not be converted into a number,
/// return None instead.
fn score_to_num(capture: Option<Captures>) -> Option<i32>{
  // If at any point this fails we should return None
  capture
    .map(|cap| cap.get(0).unwrap())
    .map(|parsed_string| {
      let maybe_score = String::from(parsed_string.as_str());
      let maybe_number = &maybe_score[1..maybe_score.len()-1];
      maybe_number.parse::<i32>().unwrap()
    })
    .map(|number| number)
}

/// Extracts a score from a trello card, based on using [] or (). If no score is found a 0 is returned
fn get_score(maybe_points: &str) -> Option<Score>{
  // this will capture on "(0)" or "[0]" where 0 is an arbitrary sized digit
  let correction = score_to_num(Regex::new(r"\[(\d+)\]")
                                .unwrap()
                                .captures(&maybe_points));

  let estimated = score_to_num(Regex::new(r"\((\d+)\)")
                               .unwrap()
                               .captures(&maybe_points));

  if let (None, None) = (estimated, correction){
    return None;
  }

  Some(
    Score{
      estimated,
      correction
    })
}


// Testable
pub fn calculate_delta(old_deck: &Deck, new_deck: &Deck) -> HashMap<String, i32>{
  let mut collection = HashMap::new();

  collection.insert("cards".to_string(), new_deck.size as i32 - old_deck.size as i32);
  collection.insert("score".to_string(), new_deck.score as i32 - old_deck.score as i32);
  collection.insert("unscored".to_string(), new_deck.unscored as i32 - old_deck.unscored as i32);
  collection.insert("estimated".to_string(), new_deck.estimated as i32 - old_deck.estimated as i32);

  collection
}

pub fn print_decks(decks: &[Deck]){
  let mut table = Table::new();

  table.add_row(row!["List", "cards", "score","estimated", "unscored"]);

  for deck in decks {
    table.add_row(row![deck.name, deck.size, deck.score, deck.estimated, deck.unscored]);
  }

  table.printstd();
}


fn get_date(database: &HashMap<String, HashMap<u64, Vec<Deck>>>, board_id: &str) -> Result<u64, Box<dyn std::error::Error>> {
  let board = match database.get(board_id){
    Some(board) => board,
    None => panic!("No board found with that id!")
  };

  let mut keys: Vec<u64> = board.keys().map(|key| key.clone()).collect();
  keys.sort();
  let items: Vec<NaiveDateTime> = keys.iter().map(|item| NaiveDateTime::from_timestamp(item.clone().try_into().unwrap(), 0)).collect();
  let index: usize = Select::new()
    .with_prompt("Select a date: ")
    .items(&items)
    .default(0)
    .interact()?;

  Ok(keys[index])
}

pub fn print_delta(decks: &[Deck], board_id: &str)-> Result<(), Box<dyn std::error::Error>>{
  let mut table = Table::new();

  table.add_row(row!["List", "cards", "score","estimated", "unscored"]);
  let mut database = get_database()?;
  let date = get_date(&database, board_id)?;

  let old_decks = database.entry(board_id.to_string())
    .or_insert(HashMap::new())
    .entry(date)
    .or_insert(Vec::new()).to_vec();

  for deck in decks {
    let matching_deck: Option<Deck> = old_decks.iter().fold(None, |match_deck, maybe_deck|
                                         if maybe_deck.name == deck.name{
                                           Some(maybe_deck.clone())
                                         }else if match_deck.is_some(){
                                           match_deck
                                         } else {
                                           None
                                         });

    match matching_deck{
      Some(old_deck) => {
        let delta = calculate_delta(&old_deck, deck);
        let cards = format!("{} ({})",deck.size, delta.get("cards").unwrap());
        let score = format!("{} ({})",deck.score, delta.get("score").unwrap());
        let estimated = format!("{} ({})",deck.estimated, delta.get("estimated").unwrap());
        let unscored = format!("{} ({})",deck.unscored, delta.get("unscored").unwrap());

        table.add_row(row![deck.name, cards, score, estimated, unscored]);
      },

      None => {
        table.add_row(row![deck.name, deck.size, deck.score, deck.estimated, deck.unscored]);
      }
    }
  }

  table.printstd();
  println!("* Printing in detailed mode. Numbers in () mark the difference from the last time card-counter was run and saved data.");
  Ok(())
}

pub mod test{
  #[allow(unused_imports)]
  use super::{get_score, Score};

  #[test]
  fn get_score_handles_curlies(){
    assert_eq!(get_score("(10)").unwrap().estimated, Some(10));

    assert_eq!(get_score("()"), None);

    assert_eq!(get_score("(z)"), None);
    assert_eq!(get_score("(10z)"), None);
  }

  #[test]
  fn get_score_handles_angles(){
    assert_eq!(get_score("[10]").unwrap().correction, Some(10));

    assert_eq!(get_score("[]"), None);

    assert_eq!(get_score("[z]"), None);
    assert_eq!(get_score("[10z]"), None);
  }

  #[test]
  fn get_score_handles_curlies_and_angles(){
    assert_eq!(get_score("[10](9)").unwrap().correction, Some(10));
    assert_eq!(get_score("[10](9)").unwrap().estimated, Some(9));
    assert_eq!(get_score("[]()"), None);

    assert_eq!(get_score("[z](9)").unwrap().estimated, Some(9));
    assert_eq!(get_score("[9](z)").unwrap().correction, Some(9) );
    assert_eq!(get_score("[](9)").unwrap().estimated, Some(9));
    assert_eq!(get_score("[9]()").unwrap().correction, Some(9));
    assert_eq!(get_score("[9z]()"), None);
  }

  #[test]
  fn get_score_handles_arbitrarily_sized_digits(){
    assert_eq!(get_score("[100000000](9)").unwrap().correction, Some(100000000));
    assert_eq!(get_score("[100000000](9)").unwrap().estimated, Some(9));
  }
}
