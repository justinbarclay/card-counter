use crate::kanban::trello::TrelloAuth;
// TODO: This is a big todo here, but we need to improve the error messaging
// across our system to make it more accessible and guide the use to the right
// action
error_chain! {
  foreign_links {
    Io(::std::io::Error);
    Reqwest(::reqwest::Error);
  }
  errors {
    InvalidAuthInformation(auth: TrelloAuth) {
      description("An error occurred while trying to authenticate with Trello.")
      display("401 Unauthorized
Please regenerate your Trello API token
https://trello.com/1/authorize?expiration=1day&name=card-counter&scope=read&response_type=token&key={}",
              auth.key)
    }
  }
}
