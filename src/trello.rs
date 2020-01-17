/// Structures for serializing and de-serializing responses from Trello
use serde::{Serialize, Deserialize};
// Unofficial struct to hold the key and token for working with the trello api
#[derive(Clone)]
pub struct Auth{
  pub key: String,
  pub token: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Board {
  pub id: String,

  pub name: String,

  pub desc: String,

  #[serde(rename = "descData")]
  pub desc_data: Option<String>,

  pub closed: Option<bool>,

  #[serde(rename = "idOrganization")]
  pub id_organization: Option<String>,

  pub pinned: Option<bool>,

  pub url: String,

  #[serde(rename = "shortUrl")]
  pub short_url: String,

  pub starred: Option<bool>,

  #[serde(rename = "enterpriseOwned")]
  pub enterprise_owned: Option<bool>
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct List {
  pub id: String,

  #[serde(rename = "idBoard")]
  pub id_board: String,

  pub name: String,

  pub color: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Card{
  pub name: String
}
