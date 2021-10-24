#!/bin/bash

export COSMOS_ACCOUNT="emoarmy";
export COSMOS_MASTER_KEY="<master-key>";

cargo run -- -f NoBurn -c --board-id 3em95wSl --database azure
