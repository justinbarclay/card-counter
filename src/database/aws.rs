/// Structures for serializing and de-serializing responses from AWS.
use crate::errors::*;

use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDb, DynamoDbClient, ListTablesInput,
                      // Structs important for create_table
                      CreateTableInput, AttributeDefinition, ProvisionedThroughput,
                      KeySchemaElement, PutItemInput, AttributeValue, GetItemInput, DescribeTableInput, DescribeTableError
};

use serde_dynamodb;
use crate::score::Deck;
use std::collections::HashMap;

pub async fn create_table(client: &DynamoDbClient) -> Result<()>{
  let table_params = CreateTableInput {
      table_name: "card-counter".to_string(),
      attribute_definitions: [
        AttributeDefinition{ attribute_name: "name".to_string(),
                             attribute_type: "S".to_string(), },
        AttributeDefinition{ attribute_name: "timestamp".to_string(),
                             attribute_type: "N".to_string(), },
      ].to_vec(),
      billing_mode: None,
      global_secondary_indexes: None,
      local_secondary_indexes: None,
      key_schema:[ KeySchemaElement{ attribute_name: "name".to_string(),
                                     key_type: "HASH".to_string(), },
                   KeySchemaElement{ attribute_name: "timestamp".to_string(),
                                     key_type: "RANGE".to_string(), } ].to_vec()
      ,
    provisioned_throughput: Some(ProvisionedThroughput{
      read_capacity_units: 1,
      write_capacity_units: 1,
    }),
    sse_specification: None,
    stream_specification: None,
    tags: None,
  };
  match client.create_table(
    table_params,
  ).await{
    Ok(ok) => println!("{:?}", ok),
    Err(err) => println!("{:?}", err)
  }
  Ok(())
}

pub async fn does_table_exist(client: &DynamoDbClient, table_name: String) -> Result<bool>{
  let table_query = client.describe_table(DescribeTableInput{
    table_name
  }).await;

  match table_query {
    Ok(_) => Ok(true),
    // We need to break down the error from

    Err(DescribeTableError) => {
      match DescribeTableError {
        ResourceNotFound => Ok(false),
        err => Err(err).chain_err(|| "Son of a bitch")
      }
    },
    Err(err) => Err(err).chain_err(|| "Error talking to ")
  }
}

pub fn add_timestamp(hash: &HashMap<String, AttributeValue>, timestamp: i32) -> Result<HashMap<String, AttributeValue>>{
  let mut deck = hash.clone();
  deck.insert("timestamp".to_string(), AttributeValue{
    n: Some(timestamp.to_string()),
    ..Default::default()
  });
  Ok(deck)
}

pub async fn test_dynamo(thing: String) -> Result<()>{
  // Boiler plate create pertinent AWS info
  let region = Region::Custom {
    name: "us-east-1".into(),
    endpoint: "http://localhost:8000".into(),
  };
  let client = DynamoDbClient::new(
    region
  );

  // Maybe create table
  match does_table_exist(&client, "card-counter".to_string()).await{
    Ok(true) => (), // Noop
    Ok(false) => create_table(&client).await?, // Create table and pass up error on failure
    Err(err) => return Err(err) // Return error
  }

  match client.list_tables(ListTablesInput::default()).await {
    Ok(output) => match output.table_names {
      Some(table_name_list) => {
        println!("Tables in database:");
        for table_name in table_name_list {
          println!("{}", table_name);
        }
      }
      None => println!("No tables in database!"),
    },
    Err(error) => {
      println!("Error: {:?}", error);
    }
  }

  // Add deck
  let dynamo_deck = serde_dynamodb::to_hashmap(&Deck{
    estimated: 10,
    score: 10,
    unscored: 20,
    name: "Test".to_string(),
    size: 30}).unwrap();
  println!("{:?}", dynamo_deck);
  println!("{:?}", add_timestamp(&dynamo_deck, 1000)?);
  let result = client.put_item(
    PutItemInput{
      item: add_timestamp(&dynamo_deck, 1000)?,
      table_name: "card-counter".to_string(),
      ..Default::default()
    }
  ).await.chain_err(|| "No more, please")?;
  println!("{:?}", result);

  // Get deck
  let mut query: HashMap<String, AttributeValue> = HashMap::new();
  query.insert("timestamp".to_string(), AttributeValue{
    n: Some(1000.to_string()),
    ..Default::default()
  });
  query.insert("name".to_string(), AttributeValue{
    s: Some("Test".to_string()),
    ..Default::default()
  });

  let response = client.get_item(GetItemInput{
             attributes_to_get: None,
             consistent_read: None,
             expression_attribute_names: None,
             key: query,
             projection_expression: None,
             return_consumed_capacity: None,
             table_name: "card-counter".to_string(),
           })
           .await
    .unwrap();
  let deck: Deck = serde_dynamodb::from_hashmap(response.item.unwrap()).unwrap();
  println!("{:?}", deck);

  //Get all decks
  let scan = client.scan(rusoto_dynamodb::ScanInput{
    table_name:"card-counter".to_string(),
    ..Default::default()
  }).await.unwrap();
  let decks: Vec<Deck> = scan.items.unwrap().iter().map(to_deck).filter_map(Result::ok).collect();
  println!("{:?}", decks);
  Ok(())
}

fn to_deck(hash: &HashMap<String, AttributeValue>) -> Result<Deck>{
  serde_dynamodb::from_hashmap(hash.clone()).chain_err(|| "Error serializing deck")
}
