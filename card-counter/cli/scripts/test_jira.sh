#!/bin/bash

export JIRA_USERNAME="justincbarclay+card-counter@gmail.com";
export JIRA_API_TOKEN="api-token"
export JIRA_URL="https://card-counter.atlassian.net"
cargo run -- -s false -k "jira"
