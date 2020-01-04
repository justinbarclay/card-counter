use std::env;

use clap::{Arg, App};

mod trello;

use trello::{List, Card};

async fn get_card_count(board_id: &str, filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>>{
  let api_key = env::var("TRELLO_API_KEY")?;
  let api_token = env::var("TRELLO_API_TOKEN")?;

  let client = reqwest::Client::new();
  let lists: Vec<List> = client.get(&format!("https://api.trello.com/1/boards/{}/lists?key={}&token={}", board_id, api_key, api_token))
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
      .get(&format!("https://api.trello.com/1/lists/{}/cards?card_fields=name&key={}&token={}", list.id, api_key, api_token))
      .send()
      .await?
      .json()
      .await?;
    println!("{}: {} cards", list.name, cards.len());
  }

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

  let matches = App::new("Card Counter")
    .version("0.1.0")
    .author("Justin Barclay <justincbarclay@gmail.com>")
    .about("Counts the number of cards that exist per list on a trello board.")
    .arg(Arg::with_name("board_id")
         .short("b")
         .long("board-id")
         .value_name("ID")
         .required(true)
         .help("The ID of the board where the cards are meant to be counted from.")
         .takes_value(true))
    .arg(Arg::with_name("filter")
         .short("f")
         .long("filter")
         .value_name("FILTER")
         .help("Removes all list with a name that contains the substring FILTER")
         .takes_value(true))
    .get_matches();
  let filter: Option<&str> = matches.value_of("filter");
  let board_id = matches.value_of("board_id").unwrap();
  get_card_count(&board_id, filter).await?;
  Ok(())

}
