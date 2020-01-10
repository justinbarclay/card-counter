use std::env;
use std::collections::HashMap;
use clap::{Arg, App};

use dialoguer::Select;

mod trello;

use trello::{Board, List, Card, Auth};

// Handles the setup for the app, mostly checking for key and token and giving the proper prompts to the user to get the right info.
fn check_for_auth() -> Option<Auth>{
  let key: String = match env::var("TRELLO_API_KEY"){
    Ok(value) => value,
    Err(_) => {
      eprintln!("Tello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
      return None
    }
  };
  let token: String = match env::var("TRELLO_API_TOKEN"){
    Ok(value) => value,
    Err(_) => {
      eprintln!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}
\n and set the token as the environment variable TRELLO_API_TOKEN", key);
      return None
    }
  };

  Some(Auth{
    key,
    token
  })
}

/// Counts the number of cards for all lists, ignoring lists whose name include the string filter, on a given board.
async fn get_card_count(auth: Auth, board_id: &str, filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>>{
  let client = reqwest::Client::new();
  let lists: Vec<List> = client.get(&format!("https://api.trello.com/1/boards/{}/lists?key={}&token={}", board_id, auth.key, auth.token))
    .send()
    .await?
    .json()
    .await?;

  let filtered_lists = lists.iter().fold(Vec::new(), |mut container, list| {
    match filter {
      Some(value) => {
        if !list.name.contains(value) {
          container.push(list);
        }
      },
      None => container.push(list)
    };

    container
  });

  for list in filtered_lists {
    let cards: Vec<Card> = client
      .get(&format!("https://api.trello.com/1/lists/{}/cards?card_fields=name&key={}&token={}", list.id, auth.key, auth.token))
      .send()
      .await?
      .json()
      .await?;
    println!("{}: {} cards", list.name, cards.len());
  }

  Ok(())
}

async fn get_board_id(auth: Auth) -> Result<String, Box<dyn std::error::Error>> {
  let client = reqwest::Client::new();
  let boards: Vec<Board> = client.get(&format!("https://api.trello.com/1/members/me/boards?key={}&token={}", auth.key, auth.token))
    .send()
    .await?
    .json()
    .await?;

  let boards2: HashMap<String, String> = boards.iter().fold(HashMap::new(), |mut collection, board| {
    collection.insert(board.name.clone(), board.id.clone());
    collection
  });

  let board_names: Vec<String> = boards2.keys().map(|key: &String| key.clone()).collect();
  let name_index: usize = Select::new()
    .with_prompt("Select a board: ")
    .items(&board_names)
    .default(0)
    .interact()?;

  Ok(boards2
     .get(&board_names[name_index])
     .unwrap()
     .to_string())
}

// Run all of network code asynchronously using tokio and await
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

  // TODO: Pull this out to yaml at some point
  let matches = App::new("Card Counter")
    .version("0.1.0")
    .author("Justin Barclay <justincbarclay@gmail.com>")
    .about("A CLI to get a quick glance of your overall status in trello by counting remaining cards in each list of a board.")
    .arg(Arg::with_name("board_id")
         .short("b")
         .long("board-id")
         .value_name("ID")
         .help("The ID of the board where the cards are meant to be counted from.")
         .takes_value(true))
    .arg(Arg::with_name("filter")
         .short("f")
         .long("filter")
         .value_name("FILTER")
         .help("Removes all list with a name that contains the substring FILTER")
         .takes_value(true))
    .get_matches();

  match  check_for_auth(){
    Some(auth) => {
      // Parse arguments, if board_id isn't found
      let filter: Option<&str> = matches.value_of("filter");
      let board_id = match matches.value_of("board_id"){
        Some(id) => id.to_string(),
        None => get_board_id(auth.clone()).await?
      };

      get_card_count(auth.clone(), &board_id, filter).await?;
      Ok(())
    },
    None => std::process::exit(1)
  }
}
