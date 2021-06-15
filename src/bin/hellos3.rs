use rusoto_s3::{ListObjectsV2Request, S3, S3Client};
use rusoto_core::Region;
// use tokio::io::

#[tokio::main]
async fn main() ->  Result<(), Box<dyn std::error::Error>> {

    let s3_client = S3Client::new(Region::EuWest1);
    let mut list_obj_req = ListObjectsV2Request{
        bucket: String::from("s3-ranch-007"),
        prefix: Some("test1000PlusS3".to_string()),
        ..Default::default()
    };
    let fut = s3_client.list_objects_v2( list_obj_req);

    let mut results = vec![];

    loop {
    let result = fut.await.unwrap();

    // for thing in response.contents.unwrap() {
    //     println!("{:#?}", thing.key.unwrap());
    // }

        match result.contents {
            Some(x) => results.extend(x),
            None => break,
        }

        list_obj_req.continuation_token = result.next_continuation_token;
        if list_obj_req.continuation_token.is_none() {
            break;
        }
    }

    println!("Listed {:?} objects.", results);

    Result::Ok(())
}

