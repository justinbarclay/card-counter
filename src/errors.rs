pub use eyre::{eyre, Context, Result};
use std::{
  error::Error,
  fmt::{self, write},
  write,
};

#[derive(Debug)]
pub enum AuthError {
  Trello(String),
  Jira(String),
}
impl Error for AuthError {}

impl fmt::Display for AuthError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self{
      AuthError::Trello(token) =>
        write!(f, "401 Unauthorized
Unauthorized request to Trello API
Please regenerate your Trello API token
https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}", token)
    ,
      AuthError::Jira(_info) => write!(f, "401 Unauthorized
Unauthorized request to Jira API")
      }
  }
}

#[derive(Debug)]
pub struct JsonParseError(pub String);

impl Error for JsonParseError {}

impl fmt::Display for JsonParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Unable to parse response from {} as JSON.", self.0)
  }
}
