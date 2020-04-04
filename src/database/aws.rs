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
use chrono::NaiveDateTime;
use dialoguer::{Confirmation, Select};
use serde_dynamodb;
use std::{collections::HashMap, convert::TryInto};

async fn create_table(client: &DynamoDbClient) -> Result<()> {
  let table_params = CreateTableInput {
    table_name: "card-counter".to_string(),
    attribute_definitions: [
      AttributeDefinition {
        attribute_name: "board_id".to_string(),
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
        attribute_name: "board_id".to_string(),
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
    Err(rusoto_core::RusotoError::Service(DescribeTableError::ResourceNotFound(_))) => {
      return Ok(false)
    }
    Err(err) => Err(err).chain_err(|| "Unable to connect to DynamoDB."),
  }
}

#[derive(Clone)]
pub struct Aws {
  client: DynamoDbClient,
}

#[async_trait]
impl Database for Aws {
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
    time_stamp: Option<u64>,
  ) -> Result<Option<Vec<Deck>>> {
    let mut query_values: HashMap<String, AttributeValue> = HashMap::new();
    let query_string = match time_stamp {
      Some(_) => "board_name = :board_name and time_stamp < :timestamp".to_string(),
      None => "board_name = :board_name".to_string(),
    };

    query_values.insert(
      ":board_name".to_string(),
      AttributeValue {
        s: Some(board_name.to_string()),
        ..Default::default()
      },
    );

    if let Some(timestamp) = time_stamp {
      query_values.insert(
        ":timestamp".to_string(),
        AttributeValue {
          n: Some(timestamp.to_string()),
          ..Default::default()
        },
      );
    }

    let query = self
      .client
      .query(QueryInput {
        consistent_read: Some(true),
        key_condition_expression: Some(query_string),
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

    self.get_decks_by_date(entries)
  }
}

impl Aws {
  // Init tries to initiate a connection to DynamoDB.
  // If it fails to connect to DynamoDB it will panic, however if it can connect and does not find a table it will then create one.
  // Should creating the table fail it will, again, panic.
  pub async fn init(config: &Config) -> Result<Self> {
    // Boiler plate create pertinent AWS info
    let region = Region::Custom {
      name: "us-east-1".into(),
      endpoint: "http://localhost:8000".into(),
    };
    let __self = Aws {
      client: DynamoDbClient::new(region),
    };

    // Maybe create table
    let table_exists = does_table_exist(&__self.client, "card-counter".to_string()).await?;

    if !table_exists {
      match Confirmation::new()
        .with_text(
          "Unable to find \"card-counter\" table in DynamoDB. Would you like to create a table?",
        )
        .interact()
        .chain_err(|| "There was a problem registering your response.")?
      {
        true => create_table(&__self.client).await?,
        false => {
          println! {"Unable to update or query table."}
          ::std::process::exit(1);
        }
      }
    }

    Ok(__self)
  }
  // TODO: This doesn't seem efficient
  pub fn get_decks_by_date(&self, board: Entries) -> Result<Option<Vec<Deck>>> {
    let mut keys: Vec<u64> = board.iter().map(|entry| entry.time_stamp).collect();

    keys.sort();
    let date = select_date(&keys).unwrap();

    match board.iter().find(|entry| entry.time_stamp == date) {
      Some(entry) => Ok(Some(entry.decks.clone())),
      None => Ok(None),
    }
  }
}

// TODO: Get rid of
fn select_date(keys: &[u64]) -> Option<u64> {
  let items: Vec<NaiveDateTime> = keys
    .iter()
    .map(|item| NaiveDateTime::from_timestamp(item.clone().try_into().unwrap(), 0))
    .collect();
  let index: usize = Select::new()
    .with_prompt("Select a date: ")
    .items(&items)
    .default(0)
    .interact()
    .unwrap();

  Some(keys[index])
}
// Helper functions
fn to_entry(hash: &HashMap<String, AttributeValue>) -> Result<Entry> {
  serde_dynamodb::from_hashmap(hash.clone()).chain_err(|| "Error serializing entry")
}
