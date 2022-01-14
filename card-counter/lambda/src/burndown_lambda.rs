mod burndown_helpers;
mod slack_helpers;
use burndown_helpers::*;
use slack_helpers::*;

use card_counter::errors::*;

use std::{collections::HashMap, str::FromStr};

use aws_lambda_events::encodings::Body;
use aws_lambda_events::event::apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse};
use http::header::{HeaderMap, CONTENT_TYPE};
use lambda::{handler_fn, Context};
use rusoto_core::Region;
use rusoto_s3::{PutObjectRequest, S3Client, S3};

use log::{error, info};

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
  _context: Context,
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

/// you can invoke the lambda with a JSON payload, which is parsed using the CustomEvent struct.
async fn my_handler(event: SlackCommand) -> Result<SlackBlock> {
  // If we use the two_weeks method we should tell them what command we ran for them
  let mut using_two_weeks = false;
  let command = match event.text.trim().is_empty() {
    true => {
      using_two_weeks = true;
      Ok(BurndownConfig::for_two_weeks_ago(
        std::env::var("DEFAULT_BOARD_ID").ok(),
      ))
    }
    false => BurndownConfig::from_str(&event.text),
  };
  let config = match command {
    Ok(config) => config,
    Err(_) => {
      return Ok(SlackBlock {
        blocks: vec![context_error(
          BurndownConfig::default().helper_string().unwrap(),
        )],
        response_type: None,
      })
    }
  };

  if let Some(help) = config.helper_string() {
    return Ok(SlackBlock {
      blocks: vec![context_error(help)],
      response_type: None,
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

  let mut blocks = vec![];

  let link = SlackMessage::markdown(format!("Click <http://{}.s3-website.{}.amazonaws.com/?date_range={}| here> to view your burndown chart.",
                       &bucket,
                       Region::default().name(),
                       &date_range));
  blocks.push(link);
  if using_two_weeks {
    let message = SlackMessage::markdown(format!("I ran the command `/card-counter burndown from {} to {} for {}` for you, if that is not what you want please type `/card-counter help` instead.",
                       &start,
                       &end,
                       &board_id));
    blocks.push(message);
  }

  Ok(SlackBlock {
    blocks,
    response_type: Some("in_channel".to_string()),
  })
}

async fn upload_chart_to_s3(chart: &str, bucket: &str, date_range: &str) -> Result<()> {
  let client = S3Client::new(Region::default());
  info!("{}", bucket);
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
