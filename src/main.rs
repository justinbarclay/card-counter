use std::env;
use clap::{Arg, App};

#[macro_use] extern crate prettytable;

mod trello;
mod score;
mod history;

use trello::Auth;
use score::{get_board_id, get_lists, build_decks, print_decks};
use history::file::update_local_database;

// Handles the setup for the app, mostly checking for key and token and giving the proper prompts to the user to get the right info.
fn check_for_auth() -> Option<Auth>{
  let key: String = match env::var("TRELLO_API_KEY"){
    Ok(value) => value,
    Err(_) => {
      eprintln!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
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

      let cards = get_lists(auth.clone(), &board_id, filter).await?;
      let decks = build_decks(auth.clone(), cards).await?;
      print_decks(&decks);

      update_local_database(&board_id, &decks)?;
      Ok(())
    },
    None => std::process::exit(1)
  }
}
