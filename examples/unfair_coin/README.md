Set environment variables:
- `TEST_BUCKET`
- `TEST_PREFIX`

Run `resources/examples/run_unfair_coin.sh` to
- Clean all objects from the S3 prefix.
- Run ABCD, saving to S3.
- Download the results to `out` folder.
- Plot stuff by running `examples/unfair_coin/plot/run.sh` (requires Python 3.12 with virtual environments).
