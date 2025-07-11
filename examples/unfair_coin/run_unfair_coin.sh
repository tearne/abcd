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

export RUST_LOG=error,abcd=info,unfair_coin=info 

cargo build --release

printf "Purge old objects (and versions) in s3://${TEST_BUCKET}/${TEST_PREFIX}\n"
printf "... 2 second pause\n"
sleep 2
cargo run --release --bin purge -- --bucket $TEST_BUCKET --prefix $TEST_PREFIX

printf "Initialising storage prefix...\n"
aws s3 sync ../../resources/empty_prefix s3://${TEST_BUCKET}/${TEST_PREFIX}/ --delete --acl bucket-owner-full-control

printf "Starting application...\n"
sleep 1
cargo run --release --example unfair_coin

printf "Downloaing the completed generations...\n"
sleep 1
aws s3 sync s3://${TEST_BUCKET}/${TEST_PREFIX}/completed ../../out/unfair_coin

printf "Plotting results...\n"
sleep 1
./plot/run.sh