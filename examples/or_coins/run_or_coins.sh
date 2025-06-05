#!/bin/bash

set -eu

# Change to script dir
SOURCE=${BASH_SOURCE[0]}
while [ -h "$SOURCE" ]; do # resolve $SOURCE until the file is no longer a symlink
  DIR=$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )
  SOURCE=$(readlink "$SOURCE")
  [[ $SOURCE != /* ]] && SOURCE=$DIR/$SOURCE # if $SOURCE was a relative symlink, we need to resolve it relative to the path where the symlink file was located
done
DIR=$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )
cd $DIR
printf "Script working dir: ${DIR}\n"

export RUST_LOG=error,abcd=info,or_coins=info 
export TEST_BUCKET=s3-ranch-007
export TEST_PREFIX=CG_testing

printf "Purge old objects (and versions) in s3://${TEST_BUCKET}/${TEST_PREFIX}\n"
printf "... 2 second pause\n"
sleep 2
cargo run --release --bin purge -- --bucket $TEST_BUCKET --prefix $TEST_PREFIX

printf "Initialising storage prefix...\n"
aws s3 sync ../../resources/empty_prefix s3://${TEST_BUCKET}/${TEST_PREFIX}/ --delete --acl bucket-owner-full-control

printf "Starting application...\n"
sleep 1
cargo run --release --example or_coins 

printf "Downloaing the completed generations...\n"
sleep 1
aws s3 sync s3://${TEST_BUCKET}/${TEST_PREFIX}/completed ../../out/or_coins

printf "Plotting results...\n"
sleep 1
uv run plot/script.py

printf "Generating Samples...\n"
sleep 1
cargo run --example or_samples

printf "Plotting Kernel...\n"
sleep 1
uv run ../or_samples/plot/script.py
