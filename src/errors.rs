pub use eyre::{eyre, Context, Result};
use std::{error::Error, fmt, write};
// TODO: This is a big todo here, but we need to improve the error messaging
// across our system to make it more accessible and guide the use to the right
// action

//     InvalidAuthInformation(auth: TrelloAuth) {
//       description("An error occurred while trying to authenticate with Trello.")
//       display("401 Unauthorized
// Please regenerate your Trello API token
// https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}",
//               auth.key)
//     }}
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
Please regenerate your Trello API token
https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}", token)
    ,
      _ => write!(f, "Unknown auth error")
      }
  }
}
