use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use abcd::Generation;
use serde::{Deserialize};
use serde_json::Value;


#[derive(Debug, Clone, Deserialize)]
pub struct Parameters {
    pub heads: f64,
}


fn score_distribution_histogram(genNo: i32, in_file_path: PathBuf,out_filepath: PathBuf) { 

    let base_json_string = &std::fs::read_to_string(&in_file_path).unwrap();
    let mut base_json: Value = serde_json::from_str(base_json_string).unwrap();


    let file = File::open(in_file_path).unwrap();
    let reader = BufReader::new(file);
    let gen: Generation<Parameters> = serde_json::from_reader(reader).unwrap();


    let heads_vec: Vec<f64> = gen.pop
        .normalised_particles()
        .iter()
        .map(|p|{
            p.parameters.heads
        })
        .collect();

    use serde_json::json;
    let overlay_json = json!({
        "particle-summary": {
            "heads": json!(heads_vec),
        }
    });

    merge(&mut base_json, &overlay_json);

    let pretty_json =  serde_json::to_string(&base_json).unwrap();
    std::fs::write(&out_filepath, pretty_json).unwrap();

}

fn main() -> std::io::Result<()> {
    let gen_dir =  Path::new("/home/tomdoherty/out/");
    for gen_no in 1..15 { 
        let in_file_path = gen_dir.join(format!("gen_{:03}.json", gen_no));
        let out_file_path = gen_dir.join(format!("gen_{:03}.json", gen_no));
        score_distribution_histogram(gen_no,in_file_path,out_file_path);
    }
    Ok(())
}

fn merge(a: &mut Value, b: &Value) {
    match (a, b) {
        (&mut Value::Object(ref mut a), &Value::Object(ref b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (a, b) => {
            *a = b.clone();
        }
    }
}