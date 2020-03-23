/// Structures for serializing and de-serializing responses from AWS.
use crate::errors::*;

use serde::{Serialize, Deserialize};

use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDb, DynamoDbClient, ListTablesInput,
                      // Structs important for create_table
                      CreateTableInput, AttributeDefinition, ProvisionedThroughput,
                      KeySchemaElement

};


pub async fn test_dynamo(thing: String) -> Result<()>{
  let region = Region::Custom {
    name: "us-east-1".into(),
    endpoint: "http://localhost:8000".into(),
  };
  let client = DynamoDbClient::new(
    region
  );
  let list_tables_input: ListTablesInput = Default::default();

  match client.list_tables(list_tables_input.clone()).await {
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
  };

  let table_params = CreateTableInput {
      table_name: "TEST".to_string(),
      attribute_definitions: [
        AttributeDefinition{ attribute_name: "id".to_string(),
                             attribute_type: "S".to_string(), },
        AttributeDefinition{ attribute_name: "timestamp".to_string(),
                             attribute_type: "N".to_string(), }
      ].to_vec(),
      billing_mode: None,
      global_secondary_indexes: None,
      local_secondary_indexes: None,
      key_schema:[ KeySchemaElement{ attribute_name: "id".to_string(),
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

  match client.list_tables(list_tables_input).await {
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

  Ok(())
}
