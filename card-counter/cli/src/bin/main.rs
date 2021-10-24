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
      Arg::with_name("kanban")
        .short("k")
        .long("kanban")
        .value_name("KANBAN")
        .help("The kanban API to get your board and card information from")
        .possible_values(&["jira", "trello"])
        .takes_value(true),
    )
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
            .required(true)
            .help("Start of the Date Range for the Burndown Chart (yyyy-mm-dd)")
            .takes_value(true),
        )
        .arg(
          Arg::with_name("end")
            .short("e")
            .long("end")
            .value_name("END-DATE")
            .required(true)
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
        )
        .arg(
          Arg::with_name("output")
            .short("o")
            .long("output")
            .value_name("OUTPUT")
            .help("Filters out all lists with a name that contains the substring FILTER")
            .possible_values(&["ascii", "csv", "svg"])
            .default_value("csv")
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

  // TODO refactor database checking into each command,
  // the command can worry about if and when to open or verify database connection
  let database: Box<dyn Database> = match Command::check_for_database(matches.value_of("database"))?
  {
    DatabaseType::Aws => Box::new(Aws::init(&Config::from_file_or_default()?).await?),
    DatabaseType::Azure => Box::new(Azure::init(&Config::from_file_or_default()?).await?),
    DatabaseType::Local => Box::new(JSON::init()?),
  };

  if let Some(matches) = matches.subcommand_matches("burndown") {
    Command::output_burndown(matches, database).await?;
  } else {
    let (board, decks) =
      Command::show_score(&Config::from_file_or_default()?, &matches, &database).await?;

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
// formatted.
#[tokio::main]
async fn main() -> Result<()> {
  run().await?;
  Ok(())
}
