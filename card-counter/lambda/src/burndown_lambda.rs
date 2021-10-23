use card_counter::{commands::burndown::{self, BurndownOptions}, database::{Database, DateRange, aws::Aws, config::{Config, trello_auth_from_env}}, errors::*, kanban::{self, Kanban, trello::{TrelloAuth, TrelloClient}}};

use std::{collections::HashMap, str::FromStr, string::ParseError};

use rusoto_core::Region;
use rusoto_s3::{PutObjectRequest, S3Client, S3};

#[macro_use]
use serde::{Deserialize, Serialize};
use aws_lambda_events::encodings::Body;
use aws_lambda_events::event::apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse};
use http::header::{HeaderMap, CONTENT_TYPE};
use lambda::{handler_fn, Context};
use serde_urlencoded;

#[macro_use]
use log::{error, info};
use simple_logger;

#[derive(Debug, Deserialize, Clone)]
struct SlackCommand {
  token: Option<String>,
  team_id: Option<String>,
  team_domain: Option<String>,
  channel_id: Option<String>,
  channel_name: Option<String>,
  user_id: Option<String>,
  user_name: Option<String>,
  command: Option<String>,
  text: String,
  api_app_id: Option<String>,
  is_enterprise_install: bool,
  response_url: Option<String>,
  trigger_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct SlackBlock {
  blocks: Vec<SlackMessage>,
  #[serde(skip_serializing_if = "Option::is_none")]
  response_type: Option<String>
}
#[derive(Debug, Serialize, Default)]
struct SlackMessage {
  #[serde(rename = "type")]
  slack_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  elements: Option<Vec<HashMap<String, String>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  text: Option<HashMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  title: Option<HashMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  image_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  alt_text: Option<String>,
}

fn context_error(message: String) -> SlackMessage {
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
#[derive(Debug, PartialEq)]
struct BurndownConfig {
  pub start: Option<String>,
  pub end: Option<String>,
  pub board_id: Option<String>,
}
impl BurndownConfig {
  fn helper_string(&self) -> Option<String> {
    if self.start.is_none() || self.end.is_none() || self.board_id.is_none() {
      Some(format!(
        "/card-counter burndown from {} to {} for {}",
        self.start.as_ref().unwrap_or(&"YYYY-MM-DD".to_string()),
        self.end.as_ref().unwrap_or(&"YYYY-MM-DD".to_string()),
        self.board_id.as_ref().unwrap_or(&"<board-id>".to_string())
      ))
    } else {
      None
    }
  }
}

impl Default for BurndownConfig {
  fn default() -> Self {
    Self {
      start: None,
      end: None,
      board_id: None,
    }
  }
}

impl FromStr for BurndownConfig {
  type Err = ParseError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut config = BurndownConfig::default();
    let tokens: Vec<&str> = s.trim().split(" ").collect();
    let mut i = 0;

    while i < tokens.len() {
      if tokens[i].to_lowercase() == "from" && i + 1 < tokens.len() {
        config.start = Some(tokens[i + 1].to_string());
      } else if tokens[i].to_lowercase() == "to" && i + 1 < tokens.len() {
        config.end = Some(tokens[i + 1].to_string());
      } else if tokens[i].to_lowercase() == "for" && i + 1 < tokens.len() {
        config.board_id = Some(tokens[i + 1].to_string());
      }
      i = i + 1;
    }
    Ok(config)
  }
}

// Often times a user will use the boards shortLink, this is an 8
// character string, but we store the index in dynamodb as the board's
// full id, a 24 character string. So we need to make sure we have the
// full id to work.
async fn get_full_board_id(board_id: String) -> Result<String> {
  let client = TrelloClient {
    client: reqwest::Client::new(),
    auth: trello_auth_from_env().unwrap()
  };

  if board_id.len() == 24 {
    Ok(board_id)
  } else {
    Ok(client.get_board(&board_id).await?.id)
  }
}

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> Result<(), Error> {
  validate_env_vars()?;
  simple_logger::SimpleLogger::new()
    .with_level(log::LevelFilter::Info)
    .init()?;

  let func = handler_fn(lambda_apigw_wrapper);
  lambda::run(func).await?;
  Ok(())
}
async fn lambda_apigw_wrapper(
  api_event: ApiGatewayProxyRequest,
  context: Context,
) -> Result<ApiGatewayProxyResponse> {
  info!("{:?}", api_event);
  let event: SlackCommand = serde_urlencoded::from_str(&api_event.body.unwrap())?;
  info!("{:?}", event);
  let response = my_handler(event).await?;
  info!("{:?}", response);
  let apigw_response = default_gateway_response(response);
  info!("{:?}", apigw_response);
  Ok(apigw_response)
}
/// you can invoke the lambda with a JSON payload, which is parsed using the CustomEvent struct.
async fn my_handler(event: SlackCommand) -> Result<SlackBlock> {
  let config = match BurndownConfig::from_str(&event.text) {
    Ok(config) => config,
    Err(_) => {
      return Ok(SlackBlock {
        blocks: vec![context_error(
          BurndownConfig::default().helper_string().unwrap(),
        )],
        response_type: None
      })
    }
  };

  if let Some(help) = config.helper_string() {
    return Ok(SlackBlock {
      blocks: vec![context_error(help)],
      response_type: None
    });
  }
  let start = config.start.unwrap();
  let end = config.end.unwrap();
  let board_id = get_full_board_id(config.board_id.unwrap()).await?;
  let chart: String = match generate_burndown_chart(&start, &end, &board_id).await {
    Ok(chart) => chart,
    Err(e) => {
      error!("{}", e);
      String::from("Error retrieving chart")
    }
  };

  let bucket = match std::env::var("BUCKET_NAME") {
    Ok(bucket) => bucket,
    Err(_) => panic!("Unable to find env variable BUCKET_NAME"),
  };

  let date_range = format!("{}_{}", &start, &end);
  upload_chart_to_s3(&chart, &bucket, &date_range).await?;

  let mut text = HashMap::new();
  text.insert("type".to_string(), "mrkdwn".to_string());
  text.insert("text".to_string(), format!("Click <http://{}.s3-website.{}.amazonaws.com/?date_range={}| here> to view your burndown chart.",
                                          &bucket,
                                          Region::default().name(),
                                          &date_range));

  let block = SlackBlock {
    blocks: vec![SlackMessage {
      slack_type: "section".to_string(),
      text: Some(text),
      ..SlackMessage::default()
    }],
    response_type: Some("in_channel".to_string())
  };
  Ok(block)
}

async fn generate_burndown_chart(start: &str, end: &str, board_id: &str) -> eyre::Result<String> {
  let client: Box<dyn Database> = Box::new(Aws::init(&Config::default()).await?);

  let range = DateRange::from_strs(start, end);
  let options = BurndownOptions {
    board_id: board_id.to_string(),
    range,
    client,
    filter: Some("NoBurn".into()),
  };
  info!("{:?}", options.board_id);
  info!("{:?}", options.range);
  let burndown = options.into_burndown().await?;
  burndown.as_svg()
}

async fn upload_chart_to_s3(chart: &str, bucket: &str, date_range: &str) -> Result<()> {
  let client = S3Client::new(Region::default());

  let filename = format!("burndown-{}.svg", date_range);
  let req = PutObjectRequest {
    bucket: bucket.to_string(),
    key: filename.clone(),
    body: Some(chart.as_bytes().to_owned().into()),
    content_type: Some("image/svg+xml".to_string()),
    ..Default::default()
  };

  let result = client.put_object(req).await.expect("Couldn't PUT object");
  info!("{:?}", result);

  Ok(())
}

fn validate_env_vars() -> Result<()> {
  if std::env::var("BUCKET_NAME").is_err() {
    panic!("Unable to find env variable BUCKET_NAME");
  }
  Ok(())
}

fn default_gateway_response(body: SlackBlock) -> ApiGatewayProxyResponse {
  let mut headers = HeaderMap::new();
  headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

  ApiGatewayProxyResponse {
    status_code: 200,
    multi_value_headers: HeaderMap::new(),
    headers,
    body: Some(Body::Text(serde_json::json!(&body).to_string())),
    is_base64_encoded: Some(false),
  }
}

#[cfg(test)]
mod test {
  use std::str::FromStr;

  use crate::BurndownConfig;

  #[test]
  fn it_makes_a_burndown_cfg() {
    let result =
      BurndownConfig::from_str("burndown from 2020-01-01 to 2020-10-01 for 3em95wSl").unwrap();
    assert_eq!(
      result,
      BurndownConfig {
        start: Some("2020-01-01".to_string()),
        end: Some("2020-10-01".to_string()),
        board_id: Some("3em95wSl".to_string())
      }
    );
  }
}
