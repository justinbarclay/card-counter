// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]
use std::{env, io::Write};

use clap::{App, Arg};

#[macro_use]
extern crate prettytable;

#[macro_use]
extern crate error_chain;

mod database;
mod errors;
mod score;
mod trello;

use chrono::NaiveDateTime;
use database::{
  aws::Aws, config::Config, format_to_burndown, get_decks_by_date, json::JSON, Database, DateRange,
  Entry, DatabaseType
};

use errors::Result;
use score::{build_decks, print_decks, print_delta, select_board, Deck};
use std::collections::HashMap;
use trello::{collect_cards, get_board, get_cards, get_lists, Auth, Board, Card};

// Handles the setup for the app, mostly checking for key and token and giving the proper prompts to the user to get the right info.
fn check_for_auth() -> Result<Option<Auth>> {
  match Config::from_file()? {
    Some(config) => Ok(Some(config.trello_auth())),
    None => Ok(auth_from_env()),
  }
}

fn auth_from_env() -> Option<Auth> {
  let key: String = match env::var("TRELLO_API_KEY") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
      return None;
    }
  };

  let token: String = match env::var("TRELLO_API_TOKEN") {
    Ok(value) => value,
    Err(_) => {
      eprintln!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN", key);
      return None;
    }
  };

  if key.is_empty() {
    eprintln!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
    return None;
  }
  if token.is_empty() {
    eprintln!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN", key);
    return None;
  }
  Some(Auth { key, token })
}

fn check_for_database(matches: &clap::ArgMatches<'_>) -> Result<DatabaseType>{
  match Config::from_file()? {
    Some(config) => Ok(config.database),
    None => match matches.value_of("database"){
      Some("aws") => Ok(DatabaseType::Aws),
      Some("local") => Ok(DatabaseType::Local),
      Some(some) => {
        println!("Unable to find database for {}. Using local database instead", some);
        Ok(DatabaseType::Local)
      }
      None => {
        println!("No database chosen, defaulting to local.");
        Ok(DatabaseType::Local)
      }
    }
  }

}

async fn show_score(
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

fn cli<'a>() -> clap::ArgMatches<'a> {
  App::new("card-counter")
    .version(env!("CARGO_PKG_VERSION"))
    .author("Justin Barclay <justincbarclay@gmail.com>")
    .about("A CLI for quickly summarizing story points in Trello lists")
    .arg(
      Arg::with_name("board_id")
        .short("b")
        .long("board-id")
        .value_name("ID")
        .help("The ID of the board where the cards are meant to be counted from")
        .takes_value(true),
    )
    .arg(
      Arg::with_name("filter")
        .short("f")
        .long("filter")
        .value_name("FILTER")
        .help("Filters out all lists with a name that contains the substring FILTER")
        .takes_value(true),
    )
    .arg(
      Arg::with_name("save")
        .short("s")
        .long("save")
        .value_name("SAVE")
        .help("Save the current entry in the database")
        .default_value("true")
        .possible_values(&["true", "false"])
        .takes_value(true),
    )
    .arg(
      Arg::with_name("database")
        .short("d")
        .long("database")
        .value_name("DATABASE")
        .help("Choose the database you want to save current request in")
        .possible_values(&["local", "aws"])
        .takes_value(true),
    )
    .arg(
      Arg::with_name("compare")
        .short("c")
        .long("compare")
        .help("Compares the current trello board with a previous entry"),
    )
    .subcommand(
      clap::SubCommand::with_name("config").about("Edit properties associated with card-counter."),
    )
    .subcommand(
      clap::SubCommand::with_name("burndown")
        .about("Parses data for a board and prints out data to be piped to gnuplot")
        .arg(
          Arg::with_name("board_id")
            .short("b")
            .long("board-id")
            .value_name("ID")
            .help("The ID of the board where the cards are meant to be counted from")
            .takes_value(true),
        )
        .arg(
          Arg::with_name("start")
            .short("s")
            .long("start")
            .value_name("START-DATE")
            .help("Start of the Date Range for the Burndown Chart (yyyy-mm-dd)")
            .takes_value(true),
        )
        .arg(
          Arg::with_name("end")
            .short("e")
            .long("end")
            .value_name("END-DATE")
            .help("End of the Date Range for the Burndown Chart (yyyy-mm-dd)")
            .takes_value(true),
        )
        .arg(
          Arg::with_name("database")
            .short("d")
            .long("database")
            .value_name("DATABASE")
            .default_value("local")
            .help("Choose the database you want to save current request in")
            .possible_values(&["local", "aws"])
            .takes_value(true),
        )
        .arg(
          Arg::with_name("filter")
            .short("f")
            .long("filter")
            .value_name("FILTER")
            .help("Filters out all lists with a name that contains the substring FILTER")
            .takes_value(true),
        ),
    )
    .get_matches()
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
  let mut entries: Vec<Entry> = client.query_entries(board.id, Some(range)).await?.unwrap();
  entries.sort();
  println!("{}", format_to_burndown(entries, filter).join("\n"));
  Ok(())
}
// Run all of network code asynchronously using tokio and await
async fn run() -> Result<()> {
  // TODO: Pull this out to yaml at some point
  let matches = cli();

  // Setting up config requires little access
  if matches.subcommand_matches("config").is_some() {
    Config::from_file_or_default()?.update_file()?;
    std::process::exit(0)
  }

  // Counting cards or generating burndown charts requires access to both Trello
  // and the database. So we've split those two commands into a separate if/else
  // block
  let auth = match check_for_auth()? {
    Some(auth) => auth,
    None => std::process::exit(1),
  };

  let database: Box<dyn Database> = match check_for_database(&matches)? {
    DatabaseType::Aws => Box::new(Aws::init(&Config::from_file_or_default()?).await?),
    DatabaseType::Local => Box::new(JSON::init()?)
  };

  if let Some(matches) = matches.subcommand_matches("burndown") {
    output_burndown(auth, matches, &database).await?;
  } else {
    let (board, decks) = show_score(auth.clone(), &matches, &database).await?;

    if matches.is_present("save") && matches.value_of("save").unwrap() == "true" {
      database
        .add_entry(Entry {
          board_id: board.id,
          time_stamp: Entry::get_current_timestamp()?,
          decks,
        })
        .await?;
    };
  }

  Ok(())
}

// The above main gives you maximum control over how the error is
// formatted. If you don't care (i.e. you want to display the full
// error during an assert) you can just call the `display_chain` method
// on the error object
#[tokio::main]
async fn main() {
  if let Err(ref e) = run().await {
    let stderr = &mut ::std::io::stderr();
    let errmsg = "Error writing to stderr";

    writeln!(stderr, "error: {}", e).expect(errmsg);

    for e in e.iter().skip(1) {
      writeln!(stderr, "caused by: {}", e).expect(errmsg);
    }

    // The backtrace is not always generated. Try to run this example
    // with `RUST_BACKTRACE=1`.
    if let Some(backtrace) = e.backtrace() {
      writeln!(stderr, "backtrace: {:?}", backtrace).expect(errmsg);
    }

    ::std::process::exit(1);
  }
}
