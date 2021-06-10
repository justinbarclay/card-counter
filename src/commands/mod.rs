use crate::{
  // database::azure::Azure,
  database::{config::Config, get_decks_by_date, Database, DatabaseType, DateRange, Entry},
  errors::Result,
  score::{build_decks, print_decks, print_delta, select_board, Deck},
  trello::{collect_cards, get_board, get_cards, get_lists, Auth, Board, Card},
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
    auth: Auth,
    matches: &clap::ArgMatches<'_>,
    client: &Box<dyn Database>,
  ) -> Result<(Board, Vec<Deck>)> {
    let filter: Option<&str> = matches.value_of("filter");
    // Parse arguments, if board_id isn't found
    let board: Board = match matches.value_of("board_id") {
      Some(id) => get_board(id, &auth).await?,
      None => select_board(&auth).await?,
    };
    let lists = get_lists(&auth, &board.id).await?;
    let cards = get_cards(&auth, &board.id).await?;
    let map_cards: HashMap<String, Vec<Card>> = collect_cards(cards);
    let decks = build_decks(lists, map_cards);

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
    auth: Auth,
    matches: &clap::ArgMatches<'_>,
    client: &Box<dyn Database>,
  ) -> Result<()> {
    let start_str = matches.value_of("start").expect("Missing start argument");
    let end_str = matches.value_of("end").expect("Missing end argument");

    let board: Board = match matches.value_of("board_id") {
      Some(id) => get_board(id, &auth).await?,
      None => select_board(&auth).await?,
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
