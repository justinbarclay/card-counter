use crate::{
  database::{
    config::{self, Config},
    get_decks_by_date, Database, DatabaseType, DateRange, Entry,
  },
  errors::Result,
  kanban::{self, jira::JiraClient, Kanban, Card, Board, List},
  score::{build_decks, print_decks, print_delta, Deck},
};
use burndown::Burndown;
use chrono::NaiveDateTime;

use std::collections::HashMap;

pub mod burndown;

pub struct Command;

impl Command {
  pub fn check_for_database(database: Option<&str>) -> Result<DatabaseType> {
    match Config::from_file()? {
      Some(config) => Ok(config.database),
      None => match database {
        Some("aws") => Ok(DatabaseType::Aws),
        Some("local") => Ok(DatabaseType::Local),
        Some("azure") => Ok(DatabaseType::Azure),
        Some(some) => {
          println!(
            "Unable to find database for {}. Using local database instead",
            some
          );
          Ok(DatabaseType::Local)
        }
        None => {
          println!("No database chosen, defaulting to local.");
          Ok(DatabaseType::Local)
        }
      },
    }
  }

  pub async fn show_score(
    config: &Config,
    matches: &clap::ArgMatches<'_>,
    client: &Box<dyn Database>,
  ) -> Result<(Board, Vec<Deck>)> {
    let filter: Option<&str> = matches.value_of("filter");
    // Parse arguments, if board_id isn't found
    let (board, decks) = match (&config.kanban, matches.value_of("kanban")) {
      (_, Some("jira")) => {
        let jira = JiraClient::init(config);
        kanban_compile_decks(jira, matches).await?
      }
      (_, Some("trello")) => trello_compile_decks(config, matches).await?,
      (config::Board::Jira(_), None) => {
        let jira = JiraClient::init(config);
        kanban_compile_decks(jira, matches).await?
      }
      _ => trello_compile_decks(config, matches).await?,
    };

    if matches.is_present("compare") {
      if let Some(old_entries) = client.query_entries(board.id.to_string(), None).await? {
        // TODO: Fix this old_decks could be empty
        let old_decks = get_decks_by_date(old_entries).unwrap();
        print_delta(&decks, &old_decks, &board.name, filter);
      } else {
        println!("Unable to retrieve any decks from the database.");
        print_decks(&decks, &board.name, filter);
      }
    } else {
      print_decks(&decks, &board.name, filter);
    }

    Ok((board, decks))
  }

  pub async fn output_burndown(
    matches: &clap::ArgMatches<'_>,
    client: &Box<dyn Database>,
  ) -> Result<()> {
    let auth = match Config::check_for_auth()? {
      Some(auth) => auth,
      None => std::process::exit(1),
    };
    let start_str = matches.value_of("start").expect("Missing start argument");
    let end_str = matches.value_of("end").expect("Missing end argument");

    let trello = kanban::trello::TrelloClient::init(&Config::from_file_or_default()?);
    let board: Board = match matches.value_of("board_id") {
      Some(id) => trello.get_board(id).await?,
      None => trello.select_board().await?,
    };

    let start = NaiveDateTime::parse_from_str(&format!("{} 0:0:0", start_str), "%F %H:%M:%S")
      .expect("Unable to parse date");
    let end = NaiveDateTime::parse_from_str(&format!("{} 0:0:0", end_str), "%F %H:%M:%S")
      .expect("Unable to parse date");
    let range = DateRange {
      start: start.timestamp(),
      end: end.timestamp(),
    };
    let filter = matches.value_of("filter");
    let entries: Vec<Entry> = client.query_entries(board.id, Some(range)).await?.unwrap();
    let burndown = Burndown::calculate_burndown(&entries, &filter);
    match matches.value_of("output") {
      Some("ascii") => burndown.as_ascii().unwrap(),
      Some("csv") => println!("{}", burndown.as_csv().join("\n")),
      Some(option) => println!("Output option {} not supported", option),
      None => println!("{}", burndown.as_csv().join("\n")),
    }
    Ok(())
  }
}

async fn trello_compile_decks(
  config: &Config,
  matches: &clap::ArgMatches<'_>,
) -> Result<(Board, Vec<Deck>)> {
  let trello = kanban::trello::TrelloClient::init(config);

  let board: kanban::Board = match matches.value_of("board_id") {
    Some(id) => trello.get_board(id).await?,
    None => trello.select_board().await?,
  };
  let lists = trello.get_lists(&board.id).await?;
  let cards = trello.get_cards(&board.id).await?;
  let map_cards: HashMap<String, Vec<Card>> = kanban::collect_cards(cards);
  let decks = kanban::build_decks(lists, map_cards);

  Ok((board, decks))
}

async fn kanban_compile_decks(
  jira: JiraClient,
  matches: &clap::ArgMatches<'_>,
) -> Result<(Board, Vec<Deck>)> {
  let board: Board = match matches.value_of("board_id") {
    Some(id) => jira.get_board(id).await?,
    None => jira.select_board().await?,
  };

  let lists = jira.get_lists(&board.id).await?;
  let cards = jira.get_cards(&board.id).await?;
  let map_cards: HashMap<String, Vec<Card>> = kanban::collect_cards(cards);
  let decks = kanban::build_decks(lists, map_cards);

  Ok((
    board,
    decks,
  ))
}
