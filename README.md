This repository contains a Rust implementation of the Approximate bayesian Computation (ABC) Sequential Monte Carlo (SMC) algorithm discussed in:
- [Inference in Epidemic Models without Likelihoods, McKinley et al (2009)](https://www.degruyter.com/document/doi/10.2202/1557-4679.1171/html)
- [On optimality of kernels for approximate Bayesian computation using sequential Monte Carlo.](https://arxiv.org/pdf/1106.6280)
- [Approximate Bayesian computation and simulation-based inference for complex stochastic epidemic models](https://projecteuclid.org/journals/statistical-science/volume-33/issue-1/Approximate-Bayesian-Computation-and-Simulation-Based-Inference-for-Complex-Stochastic/10.1214/17-STS618.full)

Key features include:
 - **Speed** - it's written in Rust.
 - **Scalability** - it coordinates distributed cluster activity using exploits AWS S3 [Strong Consistency](https://aws.amazon.com/s3/consistency/).  The  "D" in the ABC**D** stands for "distributed".
 - It can run models in any language, you just have to implement the [Model trait](https://github.com/tearne/abcd/blob/main/src/types.rs) to execute the model.  

The code has been run on 
 - clusters of up to 1500 nodes of 4 cores (using a Java model), and 
 - individual servers of up to 192 cores (using a Rust model).

Future work includes developing a simple packaging system to make it easier for modellers to run it for R models using AWS Batch.

## Developer notes
### Tests
`TEST_BUCKET=some_bucket TEST_PREFIX=abcd_tests cargo test --package abcd --lib --all-features -- storage::s3::tests::test_previous_gen_num_two --exact --show-output --nocapture`

### Run UnfairCoin
See [here](./examples/unfair_coin/readme.md)

### Purge a bucket/prefix
*WARNING*: Data will be deleted
`RUST_LOG=error,abcd=info,unfair_coin=info cargo run --release --bin purge -- --bucket some_bucket --prefix unfaircoin`

### Check number of accepted particles in latest gen in a bucket/prefix
`RUST_LOG=error,abcd=info,unfair_coin=info cargo run --release --bin check_num_particles -- --bucket some_bucket --prefix unfaircoin`
