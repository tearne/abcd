This repository contains a Rust implementation of the Approximate bayesian Computation (ABC) Sequential Monte Carlo (SMC) algorithm discussed in [Inference in Epidemic Models without Likelihoods, McKinley et al (2009)](https://www.degruyter.com/document/doi/10.2202/1557-4679.1171/html).

Key features include:
 - **Speed** - it's written in Rust.
 - **Scalability** - it coordinates distributed cluster activity using exploits AWS S3 [Strong Consistency](https://aws.amazon.com/s3/consistency/).  The  "D" in the ABCD stands for "distributed".
 - It can run models in any language, you just have to implement the [Model trait](https://github.com/tearne/abcd/blob/main/src/types.rs) to execute the model.  

The code has been run on 
 - clusters of up to 1500 nodes of 4 cores (Java model), and 
 - individual servers of up to 192 cores (Rust model).


Future work includes developing a simple packaging system to make it easier for modellers to run it for R models using AWS Batch.

## Developer notes
### Tests

`TEST_BUCKET=some_bucket TEST_PREFIX=abcd_tests cargo test --package abcd --lib --all-features -- storage::s3::tests::test_previous_gen_num_two --exact --show-output --nocapture`

### Run UnfairCoun
`TEST_BUCKET=some_bucket TEST_PREFIX=my_prefix resources/examples/run_unfair_coin.sh`

### Purge a bucket/prefix
WARNING: Data will be deleted
`RUST_LOG=error,abcd=info,unfair_coin=info cargo run --release --bin purge -- --bucket some_bucket --prefix unfaircoin`

### Check number of accpeted particles in latest gen in a bucket/prefix
`RUST_LOG=error,abcd=info,unfair_coin=info cargo run --release --bin check_num_particles -- --bucket some_bucket --prefix unfaircoin`
