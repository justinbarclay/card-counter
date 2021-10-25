use crate::{
  commands::burndown::BurndownOptions,
  database::{config::Config, get_decks_by_date, Database, DatabaseType},
  errors::Result,
  kanban::{self, init_kanban_board, Board, Card, Kanban},
  score::{print_decks, print_delta, Deck},
};

use std::collections::HashMap;

pub mod burndown;

pub struct Command;

/// Acts on commands issued by the user, often parses clap arguments to get the job done.
impl Command {
  pub fn check_for_database(database: Option<&str>) -> Result<DatabaseType> {
    match (database, Config::from_file()?) {
      (Some("aws"), _) => Ok(DatabaseType::Aws),
      (Some("local"), _) => Ok(DatabaseType::Local),
      (Some("azure"), _) => Ok(DatabaseType::Azure),
      (Some(some), _) => {
        println!(
          "Unable to find database for {}. Using local database instead",
          some
        );
        Ok(DatabaseType::Local)
      }
      (None, Some(config)) => Ok(config.database),
      (None, None) =>{
        println!("No database chosen, defaulting to local.");
        Ok(DatabaseType::Local)
      }
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

  /// Parses configuration passed in through matches
  pub async fn output_burndown(
    matches: &clap::ArgMatches<'_>,
    client: Box<dyn Database>,
  ) -> Result<()> {
    let config = match Config::from_file()? {
      Some(config) => config,
      None => panic!("clean this up"),
    };

    let kanban = init_kanban_board(&config, matches);

    let options = BurndownOptions::init_with_matches(kanban, client, matches).await?;

    let burndown = options.into_burndown().await?;

    match matches.value_of("output") {
      Some("ascii") => burndown.as_ascii().unwrap(),
      Some("csv") => println!("{}", burndown.as_csv().join("\n")),
      Some("svg") => println!("{}", burndown.as_svg().unwrap()),
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
