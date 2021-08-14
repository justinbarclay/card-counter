pub mod jira;
pub mod trello;
use std::collections::HashMap;

use crate::{
  database::config::{self, Config},
  errors::*,
  score::{get_score, Deck},
};
use jira::JiraClient;
use trello::TrelloClient;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// enum KanbanBoard {
//   Trello,
//   Jira,
// }

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Board {
  pub id: String,
  pub name: String,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct List {
  pub name: String,
  pub id: String,
  pub board_id: String,
}
#[derive(Debug)]
pub struct Card {
  pub name: String,
  pub parent_list: String,
}

pub trait KanbanClient {
  fn init() -> Self;
}

#[async_trait]
pub trait Kanban {
  async fn get_board(&self, board_id: &str) -> Result<Board>;
  async fn get_lists(&self, board_id: &str) -> Result<Vec<List>>;
  async fn get_cards(&self, board_id: &str) -> Result<Vec<Card>>;
  async fn select_board(&self) -> Result<Board>;
}

pub fn collect_cards(cards: Vec<Card>) -> HashMap<String, Vec<Card>> {
  cards.into_iter().fold(
    HashMap::new(),
    |mut collection: HashMap<String, Vec<Card>>, card: Card| {
      let list_id = card.parent_list.clone();
      collection.entry(list_id).or_default().push(card);
      collection
    },
  )
}

pub fn build_decks(
  lists: Vec<List>,
  mut associated_cards: HashMap<String, Vec<Card>>,
) -> Vec<Deck> {
  let mut decks = Vec::new();

  for list in lists {
    let cards = associated_cards.entry(list.id.clone()).or_default();
    let (score, unscored, estimated) =
      cards
        .iter()
        .fold((0, 0, 0), |(total, unscored, estimate), card| {
          if let Some(score) = get_score(&card.name) {
            if let Some(correction) = score.correction {
              (total + correction, unscored, estimate)
            } else {
              (
                total + score.estimated.unwrap(),
                unscored,
                estimate + score.estimated.unwrap(),
              )
            }
          } else {
            (total, unscored + 1, estimate)
          }
        });

    decks.push(Deck {
      list_name: list.name,
      size: cards.len(),
      score,
      unscored,
      estimated,
    });
  }

  decks
}

pub fn init_kanban_board(config: &Config, matches: &clap::ArgMatches<'_>) -> Box<dyn Kanban> {
  match matches.value_of("kanban") {
    Some("trello") => Box::new(TrelloClient::init(config)),
    Some("jira") => Box::new(JiraClient::init(config)),
    None => init_kanban_board_from_config(config),
    Some(unknown) => {
      panic!("Unknown kanban board: {}", unknown)
    }
  }
}

pub fn init_kanban_board_from_config(config: &Config) -> Box<dyn Kanban> {
  match config.kanban {
    config::KanbanBoard::Trello(_) => Box::new(TrelloClient::init(config)),
    config::KanbanBoard::Jira(_) => Box::new(JiraClient::init(config)),
  }
}
