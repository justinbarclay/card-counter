use crate::database::{Database, Entries, Entry};
/// Structures for serializing and de-serializing responses from AWS.
use crate::errors::*;
use async_trait::async_trait;
use rusoto_core::Region;
use rusoto_dynamodb::{
  AttributeDefinition,
  AttributeValue,
  // Structs important for create_table
  CreateTableInput,
  DescribeTableError,
  DescribeTableInput,
  DynamoDb,
  DynamoDbClient,
  GetItemInput,
  KeySchemaElement,
  ListTablesInput,
  ProvisionedThroughput,
  PutItemInput,
  QueryInput,
};

use super::config::Config;
use crate::score::Deck;
use serde_dynamodb;
use std::collections::HashMap;

async fn create_table(client: &DynamoDbClient) -> Result<()> {
  let table_params = CreateTableInput {
    table_name: "card-counter".to_string(),
    attribute_definitions: [
      AttributeDefinition {
        attribute_name: "board_name".to_string(),
        attribute_type: "S".to_string(),
      },
      AttributeDefinition {
        attribute_name: "time_stamp".to_string(),
        attribute_type: "N".to_string(),
      },
    ]
    .to_vec(),
    billing_mode: None,
    global_secondary_indexes: None,
    local_secondary_indexes: None,
    key_schema: [
      KeySchemaElement {
        attribute_name: "board_name".to_string(),
        key_type: "HASH".to_string(),
      },
      KeySchemaElement {
        attribute_name: "time_stamp".to_string(),
        key_type: "RANGE".to_string(),
      },
    ]
    .to_vec(),
    provisioned_throughput: Some(ProvisionedThroughput {
      read_capacity_units: 1,
      write_capacity_units: 1,
    }),
    sse_specification: None,
    stream_specification: None,
    tags: None,
  };
  match client.create_table(table_params).await {
    Ok(ok) => println!("{:?}", ok),
    Err(err) => println!("{:?}", err),
  }
  Ok(())
}

async fn does_table_exist(client: &DynamoDbClient, table_name: String) -> Result<bool> {
  let table_query = client
    .describe_table(DescribeTableInput { table_name })
    .await;

  match table_query {
    Ok(_) => Ok(true),
    // We need to break down the error from
    Err(DescribeTableError) => match DescribeTableError {
      ResourceNotFound => Ok(false),
      err => Err(err).chain_err(|| "Son of a bitch"),
    },
    Err(err) => Err(err).chain_err(|| "Error talking to "),
  }
}

pub struct Aws {
  client: DynamoDbClient,
}

#[async_trait]
impl Database for Aws {
  async fn init(config: Config) -> Result<Self> {
    // Boiler plate create pertinent AWS info
    let region = Region::Custom {
      name: "us-east-1".into(),
      endpoint: "http://localhost:8000".into(),
    };
    let __self: Aws = Aws {
      client: DynamoDbClient::new(region),
    };

    // Maybe create table
    match does_table_exist(&__self.client, "card-counter".to_string()).await {
      Ok(true) => (),                                   // Noop
      Ok(false) => create_table(&__self.client).await?, // Create table and pass up error on failure
      Err(err) => return Err(err),                      // Return error
    }
    Ok(__self)
  }

  async fn add_entry(&self, entry: Entry) -> Result<()> {
    self
      .client
      .put_item(PutItemInput {
        item: serde_dynamodb::to_hashmap(&entry).chain_err(|| "Unable to parse database entry")?,
        table_name: "card-counter".to_string(),
        ..Default::default()
      })
      .await
      .chain_err(|| "No more, please")?;

    Ok(())
  }

  async fn all_entries(&self) -> Result<Entries> {
    let scan = self
      .client
      .scan(rusoto_dynamodb::ScanInput {
        table_name: "card-counter".to_string(),
        ..Default::default()
      })
      .await
      .chain_err(|| "Error getting all decks from DynamoDb")?;

    match scan.items {
      Some(entries) => Ok(
        entries
          .iter()
          .map(to_entry)
          .filter_map(Result::ok)
          .collect(),
      ),
      None => Ok(Vec::new()),
    }
  }

  async fn get_entry(
    &self,
    board_name: String,
    time_stamp: std::primitive::u64,
  ) -> Result<Option<Entry>> {
    let mut query: HashMap<String, AttributeValue> = HashMap::new();
    query.insert(
      "time_stamp".to_string(),
      AttributeValue {
        n: Some(time_stamp.to_string()),
        ..Default::default()
      },
    );
    query.insert(
      "board_name".to_string(),
      AttributeValue {
        s: Some(board_name.to_string()),
        ..Default::default()
      },
    );

    let response = self
      .client
      .get_item(GetItemInput {
        table_name: "card-counter".to_string(),
        consistent_read: Some(true),
        key: query,
        ..Default::default()
      })
      .await
      .chain_err(|| "Uh oh can't talk to dynamodb")?;

    match response.item {
      None => Ok(None),
      Some(entry) => Ok(Some(
        serde_dynamodb::from_hashmap(entry).chain_err(|| "Error parsing dynamodb")?,
      )),
    }
  }

  async fn query_entries(
    &self,
    board_name: String,
    time_stamp: std::primitive::u64,
  ) -> Result<Entries> {
    let mut query_values: HashMap<String, AttributeValue> = HashMap::new();
    query_values.insert(
      ":board_name".to_string(),
      AttributeValue {
        s: Some(board_name.to_string()),
        ..Default::default()
      },
    );
    query_values.insert(
      ":timestamp".to_string(),
      AttributeValue {
        n: Some(time_stamp.to_string()),
        ..Default::default()
      },
    );

    let query = self
      .client
      .query(QueryInput {
        consistent_read: Some(true),
        key_condition_expression: Some(
          "board_name = :board_name and time_stamp < :timestamp".to_string(),
        ),
        expression_attribute_values: Some(query_values),
        table_name: "card-counter".to_string(),
        ..Default::default()
      })
      .await
      .chain_err(|| "Error while talking to dynamodb.")?;
    let entries: Entries = query
      .items
      .unwrap()
      .iter()
      .map(to_entry)
      .filter_map(Result::ok)
      .collect();
    Ok(entries)
  }
}

async fn get_entry(client: &DynamoDb, board_name: &str, time_stamp: u64) -> Result<Option<Entry>> {
  unimplemented!()
}
async fn get_all_entries(client: &DynamoDbClient) -> Result<Entries> {
  unimplemented!()
}

async fn get_all_after_date(
  client: &DynamoDbClient,
  board_name: &str,
  timestamp: u64,
) -> Result<Entries> {
  unimplemented!()
}

// Helper functions
fn to_entry(hash: &HashMap<String, AttributeValue>) -> Result<Entry> {
  serde_dynamodb::from_hashmap(hash.clone()).chain_err(|| "Error serializing entry")
}

// pub async fn test_dynamo(thing: String) -> Result<()> {

//   match client.list_tables(ListTablesInput::default()).await {
//     Ok(output) => match output.table_names {
//       Some(table_name_list) => {
//         println!("Tables in database:");
//         for table_name in table_name_list {
//           println!("{}", table_name);
//         }
//       }
//       None => println!("No tables in database!"),
//     },
//     Err(error) => {
//       println!("Error: {:?}", error);
//     }
//   }

//   // Add deck
//   // Create demo data
//   let deck_2 = Deck {
//     estimated: 20,
//     score: 20,
//     unscored: 40,
//     list_name: "Testing".to_string(),
//     size: 60,
//   };

//   let board = Entry {
//     board_name: "Test".to_string(),
//     time_stamp: Entry::get_current_timestamp()?,
//     decks: [deck_1].to_vec(),
//   };
//   let mut board_2 = board.clone();

//   board_2.time_stamp = Entry::get_current_timestamp()?;
//   board_2.decks.push(deck_2);

//   std::thread::sleep(std::time::Duration::from_secs(1));
//   let query_at = Entry::get_current_timestamp()?;
//   std::thread::sleep(std::time::Duration::from_secs(1));

//   add_entry(&client, &board).await?;

//   add_entry(&client, &board_2).await?;

//   // Get deck
//   println!(
//     "{:?}",
//     get_entry(&client, &board.board_name, board.time_stamp).await?
//   );

//   //Get all decks
//   let scan = get_all_entries(&client).await?;
//   println!("{:?}", scan.len());

//   // Get all decks in range
//   let entries: Entries = get_all_after_date(&client, &board.board_name, board.time_stamp).await?;
//   println!("{:?}", entries.len());
//   // println!("{:?}", get_all_after_date(&client, &board.board_name, board.time_stamp).await?);
//   Ok(())
// }
