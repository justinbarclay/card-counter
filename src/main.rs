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

use database::{
  aws::Aws,
  config::Config,
  file::{get_decks_by_date, save_local_database},
  Database, Entry,
};
use errors::Result;
use score::{build_decks, print_decks, print_delta, select_board, Deck};
use trello::{get_board, get_lists, Auth, Board};

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

async fn show_score(auth: Auth, matches: &clap::ArgMatches<'_>) -> Result<(Board, Vec<Deck>)> {
  let filter: Option<&str> = matches.value_of("filter");
  // Parse arguments, if board_id isn't found
  let board: Board = match matches.value_of("board_id") {
    Some(id) => get_board(id, &auth).await?,
    None => select_board(&auth).await?,
  };

  let cards = get_lists(&auth, &board.id).await?;
  let decks = build_decks(&auth, cards).await?;

  if matches.is_present("detailed") {
    if let Some(old_decks) = get_decks_by_date(&board.id) {
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

async fn show_score_aws(
  auth: Auth,
  matches: &clap::ArgMatches<'_>,
  client: Box<dyn Database>,
) -> Result<(Board, Vec<Deck>)> {
  let filter: Option<&str> = matches.value_of("filter");
  // Parse arguments, if board_id isn't found
  let board: Board = match matches.value_of("board_id") {
    Some(id) => get_board(id, &auth).await?,
    None => select_board(&auth).await?,
  };

  let cards = get_lists(&auth, &board.id).await?;
  let decks = build_decks(&auth, cards).await?;

  if matches.is_present("detailed") {
    if let Some(old_decks) = client.query_entries(board.id.to_string(), None).await? {
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
// Run all of network code asynchronously using tokio and await
async fn run() -> Result<()> {
  // TODO: Pull this out to yaml at some point
  let matches = App::new("Card Counter")
    .version(env!("CARGO_PKG_VERSION"))
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
         .help("Filters out all lists with a name that contains the substring FILTER")
         .takes_value(true))
    .arg(Arg::with_name("save")
         .short("s")
         .long("save")
         .value_name("SAVE")
         .help("Save the current entry in the database.")
         .default_value("true")
         .takes_value(true))
    .arg(Arg::with_name("database")
         .short("D")
         .long("database")
         .value_name("DATABASE")
         .default_value("local")
        .help("Choose the database you want to save current request in.")
         .possible_values(&["local", "aws"]))
    .arg(Arg::with_name("detailed")
         .short("d")
         .long("detailed")
         .help("Prints detailed stats for your trello lists, including the change in cards and scores from a previous run."))
    .subcommand(clap::SubCommand::with_name("config")
                .about("Edit properties associated with card-counter"))
    .get_matches();

  let config = Config::from_file_or_default()?;
  if matches.subcommand_matches("config").is_some() {
    config.update_file()?;
    std::process::exit(0)
  }

  // If we error for from trying to read the auth file then toss it up the stack otherwise deconstruct
  // Optional
  let auth = match check_for_auth()? {
    Some(auth) => auth,
    None => std::process::exit(1),
  };
  let database = Box::new(Aws::init(&config).await?);

  let (board, decks) = match matches.value_of("database") {
    Some("local") => show_score(auth, &matches).await?,
    Some("aws") => show_score_aws(auth.clone(), &matches, database.clone()).await?,
    _ => panic!("Unable to find a matching database"),
  };

  if let Some(save) = matches.value_of("save") {
    match (save, matches.value_of("database")) {
      ("true", Some("local")) => save_local_database(&board.id, &decks)?,
      ("true", Some("aws")) => {
        database
          .add_entry(Entry {
            board_id: board.id,
            time_stamp: Entry::get_current_timestamp()?,
            decks,
          })
          .await?;
      }
      _ => (),
    };
  }
  Ok(())
}

// The above main gives you maximum control over how the error is
// formatted. If you don't care (i.e. you want to display the full
// error during an assert) you can just call the `display_chain` method
// on the error object
#[allow(dead_code)]
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
