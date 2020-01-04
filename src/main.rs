use std::env;
mod trello;

use trello::{List, Card};


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let api_key = env::var("TRELLO_API_KEY")?;
  let api_token = env::var("TRELLO_API_TOKEN")?;

  let client = reqwest::Client::new();
  let lists: Vec<List> = client.get(&format!("https://api.trello.com/1/boards/3em95wSl/lists?key={}&token={}", api_key, api_token))
    .send()
    .await?
    .json()
    .await?;

  let filtered_lists = lists.iter().fold(Vec::new(), |mut container, list| {
    if !list.name.contains("[NoBurn]") {
      container.push(list);
    }
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
