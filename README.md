# abcd

## Tests
`TEST_BUCKET=some_bucket cargo test storage::s3::tests::test_purge_all_versions_of_everything -- --nocapture`

## Run UnfairCoun
`TEST_BUCKET=some_bucket resources/examples/run_unfair_coin.sh`

## Purge a bucket/prefix
WARNING: Super dangerous!
`RUST_LOG=error,abcd=info,unfair_coin=info cargo run --release --bin purge -- --bucket some_bucket --prefix unfaircoin`

## Check numer of accpeted particles in latest gen in a bucket/prefix
WARNING: Super dangerous!
`RUST_LOG=error,abcd=info,unfair_coin=info cargo run --release --bin check_num_particles -- --bucket some_bucket --prefix unfaircoin`