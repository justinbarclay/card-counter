#!/bin/bash

cross build --release --bin burndown-lambda --target x86_64-unknown-linux-musl &&\
cp ./target/x86_64-unknown-linux-musl/release/burndown-lambda ./target/x86_64-unknown-linux-musl/release/bootstrap &&\
zip -j rust.zip ./target/x86_64-unknown-linux-musl/release/bootstrap && \
sam local invoke -n ./env.json --profile default -e lambda.json
