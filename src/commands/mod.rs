use crate::{
  database::{config::Config, get_decks_by_date, Database, DatabaseType, DateRange, Entry},
  errors::Result,
  kanban::{self, init_kanban_board, Board, Card, Kanban},
  score::{print_decks, print_delta, Deck},
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
    let kanban = init_kanban_board(config, matches);
    let (board, decks) = kanban_compile_decks(kanban, matches).await?;

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
      Some("svg") => burndown.as_svg().unwrap(),
      Some(option) => println!("Output option {} not supported", option),
      None => println!("{}", burndown.as_csv().join("\n")),
    }
    Ok(())
  }
}

async fn kanban_compile_decks(
  kanban: Box<dyn Kanban>,
  matches: &clap::ArgMatches<'_>,
) -> Result<(Board, Vec<Deck>)> {
  let board: Board = match matches.value_of("board_id") {
    Some(id) => kanban.get_board(id).await?,
    None => kanban.select_board().await?,
  };

  let lists = kanban.get_lists(&board.id).await?;
  let cards = kanban.get_cards(&board.id).await?;
  let map_cards: HashMap<String, Vec<Card>> = kanban::collect_cards(cards);
  let decks = kanban::build_decks(lists, map_cards);

  Ok((board, decks))
}
