use rusoto_s3::{ListObjectsV2Request, S3, S3Client};
use rusoto_core::Region;
// use tokio::io::

#[tokio::main]
async fn main() ->  Result<(), Box<dyn std::error::Error>> {
    // let client = rusoto_sts::StsClient::new(Region::EuWest1);
    // client.

    let s3_client = S3Client::new(Region::EuWest1);
    let mut list_obj_req = ListObjectsV2Request{
        bucket: String::from("s3-ranch-007").to_owned(),
        prefix: Some("test1000PlusS3".to_string().to_owned()),
        ..Default::default()
    };
    let mut count = 0u32;
    loop{
        let result = s3_client.list_objects_v2(list_obj_req.clone());
        let response = result.await?;
        for thing in response.contents.unwrap() {
            count +=1;
            println!("{:#?}", thing.key.unwrap());
        }
        list_obj_req.continuation_token = response.next_continuation_token.clone();
        if list_obj_req.continuation_token.is_none(){
            break;
        }
    }
    println!("{:#?}", count);
    Result::Ok(())
}