Set environment variables:
- `TEST_BUCKET`
- `TEST_PREFIX`

Run `resources/examples/or_coins_olcm/run.sh` to
- Clean all objects from the S3 prefix.
- Run ABCD, saving to S3.
- Download the results to `out` folder.
- Plot stuff 
- Generate 1000 samples (around 3 different points) 
- Plot OLCM kernel & trivial kernel
