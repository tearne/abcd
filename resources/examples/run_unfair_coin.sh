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

aws s3 sync empty_prefix s3://${TEST_BUCKET}/${TEST_PREFIX}/ --delete --acl bucket-owner-full-control

export RUST_LOG=error,abcd=info,unfair_coin=info 
cargo run --example unfair_coin

aws s3 sync s3://${TEST_BUCKET}/${TEST_PREFIX} ../out