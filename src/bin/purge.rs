use abcd::storage::s3::S3System;
use structopt::StructOpt;


#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Purge {
    #[structopt(short, long)]
    bucket: String,
    #[structopt(short, long)]
    prefix: String,
}

fn main() {
    env_logger::init();

    let purge = Purge::from_args();
    println!("{:#?}", purge);

    let s3 = S3System::new(
        purge.bucket,
        purge.prefix,
    ).unwrap();

    s3.purge_all_versions_of_everything_in_prefix().unwrap();
}