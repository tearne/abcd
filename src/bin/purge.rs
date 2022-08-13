use abcd::storage::s3::S3System;
use clap::Parser;
use tokio::runtime::Runtime;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Purge {
    #[clap(short, long)]
    bucket: String,
    #[clap(short, long)]
    prefix: String,
}

/// Run the purger to delete all versions of all objects
/// 
/// ```
/// export RUST_LOG=error,abcd=info
/// cargo run --release --bin purge -- --bucket $TEST_BUCKET --prefix $TEST_PREFIX
/// ```
fn main() {
    env_logger::init();

    let purge = Purge::from_args();
    println!("{:#?}", purge);

    let runtime = Runtime::new().unwrap();
    let handle = runtime.handle();

    let s3 = S3System::new(
        purge.bucket,
        purge.prefix,
        handle.clone()
    ).unwrap();

    s3.purge_all_versions_of_everything_in_prefix().unwrap();
}