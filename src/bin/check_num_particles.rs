use abcd::storage::{s3::S3System, Storage};
use clap::Parser;
use tokio::runtime::Runtime;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CheckNumParticles {
    #[clap(short, long)]
    bucket: String,
    #[clap(short, long)]
    prefix: String,
}

/// Run the check to find out how many accepted particles in current fit
///
/// ```
/// export RUST_LOG=error,abcd=info
/// cargo run --release --bin checknumparticles -- --bucket $TEST_BUCKET --prefix $TEST_PREFIX
/// ```
fn main() {
    env_logger::init();

    let check_num_particles = CheckNumParticles::from_args();
    //println!("{:#?}", checkNumParticles);

    let runtime = Runtime::new().unwrap();
    let handle = runtime.handle();

    let s3 = S3System::new(check_num_particles.bucket, check_num_particles.prefix, handle.clone()).unwrap();

    println!("Number of accepted particles in current gen: {:#?}", s3.num_accepted_particles().unwrap());
}
