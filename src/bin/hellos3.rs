use rusoto_s3::{ListObjectsV2Request, S3, S3Client};
use rusoto_core::Region;
// use tokio::io::

#[tokio::main]
async fn main() ->  Result<(), Box<dyn std::error::Error>> {
    // let client = rusoto_sts::StsClient::new(Region::EuWest1);
    // client.

    let s3_client = S3Client::new(Region::EuWest1);
    let fut = s3_client.list_objects_v2(ListObjectsV2Request{
        bucket: String::from("s3-ranch-007"),
        prefix: Some("example/gen_002/".to_string()),
        ..Default::default()
    });

    let response = fut.await?;

    for thing in response.contents.unwrap() {
        println!("{:#?}", thing.key.unwrap());
    }

    

    Result::Ok(())
    //TODO this will only list up to 1000... what about the rest?

    // s3_client.put_object(List {
    //     bucket: String::from("s3-ranch-007"),
    //     key: "@types.json".to_string(),
    //     body: Some(json::stringify(paths)),
    //     acl: Some("public-read".to_string()),
    //     ..Default::default()
    // }).sync().expect("could not upload");
}