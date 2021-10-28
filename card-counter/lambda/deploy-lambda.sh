#!/bin/bash
cur_dir=$PWD

cd ../../
cross build --release -p burndown-lambda --target x86_64-unknown-linux-musl

cp ./target/x86_64-unknown-linux-musl/release/burndown-lambda ./target/x86_64-unknown-linux-musl/release/bootstrap

zip -j card-counter/lambda/rust.zip . /target/x86_64-unknown-linux-musl/release/bootstrap

cd $cur_dir
sam build
sam deploy
