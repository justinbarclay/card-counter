// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]
use std::io::Write;

use clap::{App, Arg};

use card_counter::{
  commands::Command,
  database::{aws::Aws, azure::Azure, config::Config, json::JSON, Database, DatabaseType, Entry},
  errors::Result,
};

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
        .possible_values(&["local", "aws", "azure"])
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
    .subcommand(clap::SubCommand::with_name("test").about("A way to quickly test out code."))
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
            .possible_values(&["local", "aws", "azure"])
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

// Run all of network code asynchronously using tokio and await
async fn run() -> Result<()> {
  // TODO: Pull this out to yaml at some point
  let matches = cli();

  // Setting up config requires little access
  if matches.subcommand_matches("config").is_some() {
    Config::from_file_or_default()?.update_file()?;
    std::process::exit(0)
  }

  if matches.subcommand_matches("test").is_some() {
    Command::test().await?;
    std::process::exit(0)
  }

  // Counting cards or generating burndown charts requires access to both Trello
  // and the database. So we've split those two commands into a separate if/else
  // block
  let auth = match Config::check_for_auth()? {
    Some(auth) => auth,
    None => std::process::exit(1),
  };

  // TODO refactor database checking into each command,
  // the command can worry about if and when to open or verify database connection
  let database: Box<dyn Database> = match Command::check_for_database(matches.value_of("database"))?
  {
    DatabaseType::Aws => Box::new(Aws::init(&Config::from_file_or_default()?).await?),
    DatabaseType::Azure => Box::new(Azure::init(&Config::from_file_or_default()?).await?),
    DatabaseType::Local => Box::new(JSON::init()?),
  };

  if let Some(matches) = matches.subcommand_matches("burndown") {
    Command::output_burndown(auth, matches, &database).await?;
  } else {
    let (board, decks) = Command::show_score(auth.clone(), &matches, &database).await?;

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
