// TODO: Refactor Errors... soon
use crate::{
  database::{config::Config, Database, Entries, Entry},
  score::Deck,
};
use azure_core::HttpClient;
use azure_cosmos::prelude::{collection::*, *};
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{borrow::Borrow, collections::HashMap, env, sync::Arc};

/*
Structures for serializing and de-serializing responses from Azure.
*/
use crate::errors::*;

use async_trait::async_trait;

// async fn query_azure() -> Result<()> {
//   let authorization_token = AuthorizationToken::new_master(&master_key)?;
// }
pub struct Azure {
  client: CosmosClient,
  database_name: String,
  collection_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CosmosEntry {
  id: String,
  board_id: String,
  timestamp: i64,
  decks: Vec<Deck>,
}

impl PartialEq for CosmosEntry {
  fn eq(&self, other: &Self) -> bool {
    self.timestamp == other.timestamp && self.board_id == other.board_id
  }
}

impl Eq for CosmosEntry {}

impl From<Entry> for CosmosEntry {
  fn from(entry: Entry) -> Self {
    CosmosEntry {
      id: format!("{}-{}", entry.board_id, entry.time_stamp),
      board_id: entry.board_id,
      timestamp: entry.time_stamp,
      decks: entry.decks.clone(),
    }
  }
}

impl From<CosmosEntry> for Entry {
  fn from(entry: CosmosEntry) -> Self {
    Entry {
      time_stamp: entry.timestamp,
      board_id: entry.board_id,
      decks: entry.decks,
    }
  }
}

impl From<&CosmosEntry> for Entry {
  fn from(entry: &CosmosEntry) -> Self {
    Entry {
      time_stamp: entry.timestamp,
      board_id: entry.board_id.clone(),
      decks: entry.decks.clone(),
    }
  }
}

#[async_trait]
impl Database for Azure {
  async fn add_entry(&self, entry: Entry) -> Result<()> {
    let document = Document::new(CosmosEntry::from(entry));
    let entry = self
      .client
      .clone()
      .into_database_client(self.database_name.clone())
      .into_collection_client(self.collection_name.clone())
      .create_document()
      .execute_with_partition_key(&document, &document.document.board_id)
      .await
      .map_err(|e| {
        eprintln!("{:?}", e);
        "Unable to add entry"
      })?;

    Ok(())
  }

  async fn all_entries(&self) -> Result<Option<Entries>> {
    let documents = self
      .client
      .clone()
      .into_database_client(self.database_name.clone())
      .into_collection_client(self.collection_name.clone())
      .list_documents()
      .execute::<CosmosEntry>()
      .await
      .map_err(|e| {
        eprintln!("{:?}", e);
        "Unable to get documents from CosmoDB"
      })?
      .documents;
    let entries: Entries = documents
      .iter()
      .map(|doc| Entry::from(doc.document.clone()))
      .collect();

    Ok(Some(entries))
  }

  async fn get_entry(&self, board_name: String, time_stamp: i64) -> Result<Option<Entry>> {
    let results = self
      .client
      .clone()
      .into_database_client(self.database_name.clone())
      .into_collection_client(self.collection_name.clone())
      .query_documents()
      // .consistency_level(ConsistencyLevel::Bounded)
      .execute::<CosmosEntry, _>(&format!(
        "SELECT * FROM c WHERE c.board_id = \"{}\" AND c.timestamp = {} ORDER BY c._ts DESC OFFSET 0 LIMIT 1",
        board_name, time_stamp
      ))
      .await
      .map_err(|e| {
        eprintln!("{:?}", e);
        "Unable to get documents from CosmoDB"
      })?.into_raw().results;
    if let Some(cosmo_entry) = results.first() {
      Ok(Some(Entry::from(cosmo_entry.to_owned())))
    } else {
      Ok(None)
    }
  }

  async fn query_entries(
    &self,
    board_name: String,
    date_range: Option<super::DateRange>,
  ) -> Result<Option<Entries>> {
    let query = match date_range {
      Some(range) => format!(
        "SELECT * FROM c WHERE c.board_id = \"{}\" AND (c.timestamp BETWEEN {} AND {}) ORDER BY c.timestamp DESC",
        board_name, range.start, range.end),
      None => format!(
        "SELECT * FROM c WHERE c.board_id = \"{}\" ORDER BY c.timestamp DESC", board_name)
    };

    let results = self
      .client
      .clone()
      .into_database_client(self.database_name.clone())
      .into_collection_client(self.collection_name.clone())
      .query_documents()
      .query_cross_partition(true)
      .parallelize_cross_partition_query(true)
      .execute::<CosmosEntry, _>(&query)
      .await
      .map_err(|e| {
        eprintln!("{:?}", e);
        "Unable to get documents from CosmoDB"
      })?
      .into_raw()
      .results;

    Ok(Some(
      results
        .iter()
        .map(|entry: &CosmosEntry| Entry::from(entry))
        .collect(),
    ))
  }
}
impl Azure {
  // I _hate_ this method. But ErrorChain is not working so it's hard
  // to have things flow nicely right now.
  pub async fn init(config: &Config) -> Result<Self> {
    println!("Azure");
    let auth = auth_from_env().chain_err(|| "Unable to find Azure Master Key")?;
    let auth_token = permission::AuthorizationToken::primary_from_base64(
      auth.get("COSMOS_MASTER_KEY").unwrap_or(&"".to_string()),
    )
    .chain_err(|| "Unable to parse primary token")?;
    let account_name = match auth.get("COSMOS_ACCOUNT") {
      Some(v) => v.clone(),
      None => "".to_string(),
    };
    let client = reqwest::Client::builder()
      .danger_accept_invalid_certs(true)
      .build()?;
    let http_client: Arc<dyn HttpClient> = Arc::new(client);
    let client = CosmosClient::new(account_name, auth_token, CosmosOptions::default());

    let database_details = config.database_configuration.as_ref().chain_err(|| "No details set for Azure database in config file. Please run 'card-counter config' to set database and container names.")?;
    let azure = Azure {
      client,
      database_name: database_details
        .database_name
        .as_ref()
        .chain_err(|| {
          "No database name set. Please run 'card-counter config' to set the database name"
        })?
        .clone(),
      collection_name: database_details
        .container_name
        .clone()
        .chain_err(|| {
          "No container name set. Please run 'card-counter config' to set the container name"
        })?
        .clone(),
    };

    let db_exist = does_database_exist(&azure).await?;
    if !db_exist {
      match dialoguer::Confirm::new()
        .with_prompt(
          "Unable to find \"card-counter\" database in CosmosDB. Would you like to create a database?",
        )
        .interact()
        .chain_err(|| "There was a problem registering your response.")?
      {
        true => azure.create_database().await?,
        false => {
          eprintln! {"Unable to update or query CosmosDB."}
          ::std::process::exit(1);
        }
      }
    }

    let collection_exist = does_collection_exist(&azure, &"card-counter").await?;
    if !collection_exist {
      match dialoguer::Confirm::new()
        .with_prompt(
          "Unable to find \"card-counter\" collection in CosmosDB. Would you like to create collection?",
        )
        .interact()
        .chain_err(|| "There was a problem registering your response.")?
      {
        true => azure.create_collection().await?,
        false => {
          eprintln! {"Unable to update or query CosmosDB."}
          ::std::process::exit(1);
        }
      }
    }
    Ok(azure)
  }

  async fn create_collection(&self) -> Result<()> {
    let indexes = IncludedPathIndex {
      kind: KeyKind::Hash,
      data_type: DataType::String,
      precision: Some(3),
    };

    let path = IncludedPath {
      path: "/*".to_owned(),
      indexes: Some(vec![indexes]),
    };

    let ip = IndexingPolicy {
      automatic: true,
      indexing_mode: IndexingMode::Consistent,
      included_paths: vec![path],
      excluded_paths: vec![],
    };

    self
      .client
      .clone()
      .into_database_client("card-counter")
      .create_collection(
        azure_core::Context::new(),
        "card-counter",
        CreateCollectionOptions::new("/board_id").indexing_policy(ip),
      )
      .await
      .map_err(|_| "Unable to create CosmosDB collection.")?;

    Ok(())
  }

  async fn create_database(&self) -> Result<()> {
    self
      .client
      .create_database(
        azure_core::Context::new(),
        "card-counter",
        CreateDatabaseOptions::new(),
      )
      .await
      .map_err(|_| "Unable to create Cosmos DB")?;
    Ok(())
  }
}

async fn does_database_exist(azure: &Azure) -> Result<bool> {
  let databases = azure
    .client
    .list_databases()
    .execute()
    .await
    .map_err(|e| format!("Unable to connect to Azure CosmosDB\n {}", e))?
    .databases;

  match databases.iter().find_map(|database| {
    if &database.id == &azure.database_name {
      Some(database)
    } else {
      None
    }
  }) {
    Some(_db) => Ok(true),
    None => Ok(false),
  }
}

async fn does_collection_exist(azure: &Azure, name: &str) -> Result<bool> {
  let collections = azure
    .client
    .clone()
    .into_database_client("card-counter")
    .list_collections()
    .execute()
    .await
    .map_err(|_| "There was an error talking to CosmosDB")?
    .collections;

  match collections.iter().find_map(|collecation| {
    if &collecation.id == name {
      Some(collecation)
    } else {
      None
    }
  }) {
    Some(_collection) => Ok(true),
    None => Ok(false),
  }
}

fn auth_from_env() -> Option<HashMap<String, String>> {
  let mut auth: HashMap<String, String> = HashMap::new();
  match env::var("COSMOS_ACCOUNT") {
    Ok(value) => auth.insert("COSMOS_ACCOUNT".into(), value),
    Err(_) => {
      eprintln!("Cosmos API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"AZURE_ACCOUNT\"");
      return None;
    }
  };

  match env::var("COSMOS_MASTER_KEY") {
    Ok(value) => auth.insert("COSMOS_MASTER_KEY".into(), value),
    Err(_) => {
      eprintln!("AZURE_MASTER_KEY is missing. Please set the key as the environment variable AZURE_MASTER_KEY");
      return None;
    }
  };

  // TODO: reimplement empty check
  // if key.is_empty() {
  //   eprintln!("Trello API key not found. Please visit https://trello.com/app-key and set it as the environment variable \"TRELLO_API_KEY\"");
  //   return None;
  // }
  // if token.is_empty() {
  //   eprintln!("Trello API token is missing. Please visit https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}\n and set the token as the environment variable TRELLO_API_TOKEN", key);
  //   return None;
  // }
  Some(auth)
}

pub mod test {

  #[allow(unused_imports)]
  use super::{CosmosEntry, Entry};

  #[test]
  fn entry_and_cosmos_entry_can_be_equal() {
    let entry = Entry {
      board_id: "1".to_string(),
      time_stamp: 1,
      decks: vec![],
    };

    let cosmos = CosmosEntry {
      id: "1".to_string(),
      board_id: "1".to_string(),
      timestamp: 1,
      decks: vec![],
    };

    assert_eq!(&entry, &cosmos.clone().into());
    assert_eq!(&cosmos, &entry.into());
  }
}
