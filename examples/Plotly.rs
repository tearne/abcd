use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use abcd::Generation;
use plotly::common::Mode;
use plotly::{Plot, Histogram, ImageFormat};
use serde::{Deserialize};
use abcd::error::ABCDResult;
use statrs::statistics::Statistics;

// use plotly::box_plot::{BoxMean, BoxPoints};
// use plotly::common::{ErrorData, ErrorType, Line, Marker, Mode, Orientation, Title};
// use plotly::histogram::{Bins, Cumulative, HistFunc, HistNorm};
// use plotly::layout::{Axis, BarMode, BoxMode, Layout, Margin};

#[derive(Debug, Clone, Deserialize)]
pub struct Parameters {
    pub heads: f64,
}


fn score_distribution_histogram() { //-> Result<()>  {

    let gen_dir =  Path::new("/home/tomdoherty/out/");
    let gen_no = 15;
    let file_path = gen_dir.join(format!("gen_{:03}.json", gen_no));
    let file = File::open(file_path).unwrap();
    let reader = BufReader::new(file);
    let gen: Generation<Parameters> = serde_json::from_reader(reader).unwrap();


    let json = json!({
        "particle-summary": [
            "heads": [gen.]
        ]
    });


   // let particles = gen.pop.normalised_particles();
    let score_distribution: Vec<f64> = gen
        .pop
        .normalised_particles()
        .iter()
        .map(|particle| {
            let mean_scores: f64 = particle.scores.clone().mean();
            mean_scores
        })
        .collect();

    let trace = Histogram::new(score_distribution).name("Score Distribution");
    let mut plot = Plot::new();
    plot.add_trace(trace);
    // The following will save the plot in all available formats and show the plot.
    plot.save("score", ImageFormat::PNG,  1024, 680, 1.0);
    plot.show();
}

fn main() -> std::io::Result<()> {
    score_distribution_histogram();
    Ok(())
}