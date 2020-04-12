//
use crate::database::{Database, Entries, Entry};
// Structures for serializing and de-serializing responses from AWS.
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
  ProvisionedThroughput,
  PutItemInput,
  QueryInput,
};

use super::{config::Config, DateRange};

use dialoguer::Confirmation;

use std::collections::HashMap;

/////////////////////////
// Helper Functions
/////////////////////////
// Functions for interacting with and dealing with
// DynamoDB

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
    Ok(_) => (),
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
    Err(rusoto_core::RusotoError::Service(DescribeTableError::ResourceNotFound(_))) => Ok(false),
    Err(err) => Err(err),
  }
  .chain_err(|| "Unable to connect to DynamoDB.")
}

fn to_entry(hash: &HashMap<String, AttributeValue>) -> Result<Entry> {
  serde_dynamodb::from_hashmap(hash.clone()).chain_err(|| "Error serializing entry")
}

/////////////////////////
// AWS
/////////////////////////
#[derive(Clone)]
pub struct Aws {
  client: DynamoDbClient,
}

#[async_trait]
impl Database for Aws {
  /// Adds an entry into DynamoDB. May return an error if there are problems parsing an Entry into a hashmap or when trying to talk to DynamoDB
  async fn add_entry(&self, entry: Entry) -> Result<()> {
    self
      .client
      .put_item(PutItemInput {
        item: serde_dynamodb::to_hashmap(&entry).chain_err(|| "Unable to parse database entry")?,
        table_name: "card-counter".to_string(),
        ..Default::default()
      })
      .await
      .chain_err(|| "Unable to add entry to DynamoDB.")?;

    Ok(())
  }

  /// Retrieves all entries for the `card-counter` table. It will return an error if there was a problem talking to DynamoDB.
  async fn all_entries(&self) -> Result<Option<Entries>> {
    let scan = self
      .client
      .scan(rusoto_dynamodb::ScanInput {
        table_name: "card-counter".to_string(),
        ..Default::default()
      })
      .await
      .chain_err(|| "Error getting all decks from DynamoDb")?;

    match scan.items {
      Some(entries) => Ok(Some(
        entries
          .iter()
          .map(to_entry)
          .filter_map(Result::ok)
          .collect(),
      )),
      None => Ok(None),
    }
  }

  /// Searches DynamoDB for an entry that contains board_id and time_stamp. It will return an error if there was an issue talking to DynamoDB or parsing the returned Entry.
  async fn get_entry(&self, board_name: String, time_stamp: u64) -> Result<Option<Entry>> {
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
      .chain_err(|| "Unable to talk to DynamoDB")?;

    match response.item {
      None => Ok(None),
      Some(entry) => Ok(Some(
        serde_dynamodb::from_hashmap(entry).chain_err(|| "Error parsing entry.")?,
      )),
    }
  }

  /// Returns a selection of Entries that match the board_id and optionally all entries with board_id and have a timestampe greater than time_stamp. It can return an error when prompting a user or when talking to DynamoDB.
  async fn query_entries(
    &self,
    board_id: String,
    date_range: Option<DateRange>,
  ) -> Result<Option<Entries>> {
    let mut query_values: HashMap<String, AttributeValue> = HashMap::new();
    let query_string = match date_range {
      Some(_) => "board_id = :board_id and time_stamp <= :end and time_stamp => :start".to_string(),
      None => "board_id = :board_id ".to_string(),
    };

    query_values.insert(
      ":board_id".to_string(),
      AttributeValue {
        s: Some(board_id.to_string()),
        ..Default::default()
      },
    );

    if let Some(range) = date_range {
      query_values.insert(
        ":start".to_string(),
        AttributeValue {
          n: Some(range.start.to_string()),
          ..Default::default()
        },
      );

      query_values.insert(
        ":end".to_string(),
        AttributeValue {
          n: Some(range.end.to_string()),
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

    Ok(Some(entries))
  }
}

impl Aws {
  /// Init tries to initiate a connection to DynamoDB.
  /// It will look to see the `card-counter` table exists and if it doesn't find one, it will prompt the user if it wants to create a new table in DynamoDB.
  /// It will error if it can't talk to DynamoDB or if it can't find the `card-counter` table and the user declines to create one.
  pub async fn init(_config: &Config) -> Result<Self> {
    // Boiler plate create pertinent AWS info

    let region = Region::default();

    let aws = Aws {
      client: DynamoDbClient::new(region),
    };
    // Maybe create table
    let table_exists = does_table_exist(&aws.client, "card-counter".to_string()).await?;

    if !table_exists {
      match Confirmation::new()
        .with_text(
          "Unable to find \"card-counter\" table in DynamoDB. Would you like to create a table?",
        )
        .interact()
        .chain_err(|| "There was a problem registering your response.")?
      {
        true => create_table(&aws.client).await?,
        false => {
          println! {"Unable to update or query table."}
          ::std::process::exit(1);
        }
      }
    }

    Ok(aws)
  }
}
