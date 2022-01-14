/// A set of helper functions for dealing with generating burndown charts
use card_counter::{commands::burndown::BurndownOptions,
                   database::{Database, DateRange, aws::Aws, config::{Config, trello_auth_from_env}},
                   errors::*,
                   kanban::{Kanban, trello::TrelloClient}};
use std::{str::FromStr, string::ParseError};
use chrono::prelude::*;
use log::{info};


#[derive(Debug, PartialEq)]
pub struct BurndownConfig {
  pub start: Option<String>,
  pub end: Option<String>,
  pub board_id: Option<String>,
}
impl BurndownConfig {
  pub fn helper_string(&self) -> Option<String> {
    if self.start.is_none() || self.end.is_none() || self.board_id.is_none() {
      Some(format!(
        "/card-counter burndown from {} to {} for {}",
        self.start.as_ref().unwrap_or(&"YYYY-MM-DD".to_string()),
        self.end.as_ref().unwrap_or(&"YYYY-MM-DD".to_string()),
        self.board_id.as_ref().unwrap_or(&"<board-id>".to_string())
      ))
    } else {
      None
    }
  }
  pub fn for_two_weeks_ago(board_id: Option<String>) -> BurndownConfig{
    let today = Utc::now().timestamp() + (24 * 3600);
    let two_weeks_ago = today - (2 * 7 * 24 * 3600);
    BurndownConfig {
      start:  Some(Utc.timestamp(two_weeks_ago, 0).format("%Y-%m-%d").to_string()),
      end: Some(Utc.timestamp(today, 0).format("%Y-%m-%d").to_string()),
      board_id: board_id
    }
  }
}

impl Default for BurndownConfig {
  fn default() -> Self {
    Self {
      start: None,
      end: None,
      board_id: None,
    }
  }
}

impl FromStr for BurndownConfig {
  type Err = ParseError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut config = BurndownConfig::default();
    let tokens: Vec<&str> = s.trim().split(' ').collect();
    let mut i = 0;

    while i < tokens.len() {
      if tokens[i].to_lowercase() == "from" && i + 1 < tokens.len() {
        config.start = Some(tokens[i + 1].to_string());
      } else if tokens[i].to_lowercase() == "to" && i + 1 < tokens.len() {
        config.end = Some(tokens[i + 1].to_string());
      } else if tokens[i].to_lowercase() == "for" && i + 1 < tokens.len() {
        config.board_id = Some(tokens[i + 1].to_string());
      }
      i += 1;
    }
    Ok(config)
  }
}

// Often times a user will use the boards shortLink, this is an 8
// character string, but we store the index in dynamodb as the board's
// full id, a 24 character string. So we need to make sure we have the
// full id to work.
pub async fn get_full_board_id(board_id: String) -> Result<String> {
  let client = TrelloClient {
    client: reqwest::Client::new(),
    auth: trello_auth_from_env().unwrap()
  };

  if board_id.len() == 24 {
    Ok(board_id)
  } else {
    Ok(client.get_board(&board_id).await?.id)
  }
}

pub fn validate_env_vars() -> Result<()> {
  if std::env::var("BUCKET_NAME").is_err() {
    panic!("Unable to find env variable BUCKET_NAME");
  }
  Ok(())
}


pub async fn generate_burndown_chart(start: &str, end: &str, board_id: &str) -> eyre::Result<String> {
  let client: Box<dyn Database> = Box::new(Aws::init(&Config::default()).await?);

  let range = DateRange::from_strs(start, end);
  let options = BurndownOptions {
    board_id: board_id.to_string(),
    range,
    client,
    filter: Some("NoBurn".into()),
  };
  info!("{:?}", options.board_id);
  info!("{:?}", options.range);
  let burndown = options.into_burndown().await?;
  burndown.as_svg()
}

#[cfg(test)]
mod test {
  use std::str::FromStr;

  use crate::BurndownConfig;

  #[test]
  fn it_makes_a_burndown_cfg() {
    let result =
      BurndownConfig::from_str("burndown from 2020-01-01 to 2020-10-01 for 3em95wSl").unwrap();
    assert_eq!(
      result,
      BurndownConfig {
        start: Some("2020-01-01".to_string()),
        end: Some("2020-10-01".to_string()),
        board_id: Some("3em95wSl".to_string())
      }
    );
  }
}
