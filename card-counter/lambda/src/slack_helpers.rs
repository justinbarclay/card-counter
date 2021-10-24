/// Helper functions for dealing with Slack
use std::{collections::HashMap};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct SlackCommand {
  pub token: Option<String>,
  pub team_id: Option<String>,
  pub team_domain: Option<String>,
  pub channel_id: Option<String>,
  pub channel_name: Option<String>,
  pub user_id: Option<String>,
  pub user_name: Option<String>,
  pub command: Option<String>,
  pub text: String,
  pub api_app_id: Option<String>,
  pub is_enterprise_install: bool,
  pub response_url: Option<String>,
  pub trigger_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SlackBlock {
  pub blocks: Vec<SlackMessage>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub response_type: Option<String>
}
#[derive(Debug, Serialize, Default)]
pub struct SlackMessage {
  #[serde(rename = "type")]
  pub slack_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub elements: Option<Vec<HashMap<String, String>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub text: Option<HashMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<HashMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub image_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub alt_text: Option<String>,
}

pub fn context_error(message: String) -> SlackMessage {
  let mut context: HashMap<String, String> = HashMap::new();
  context.insert("type".to_string(), "mrkdwn".to_string());
  context.insert(
    "text".to_string(),
    format!(
      "I'm sorry, I didn't understand your request. Try formatting your request like\n{}",
      message
    ),
  );
  SlackMessage {
    slack_type: "context".to_string(),
    elements: Some(vec![context]),
    text: None,
    ..SlackMessage::default()
  }
}
