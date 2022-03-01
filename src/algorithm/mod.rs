use crate::{Generation, Model, Particle};
use crate::ABCDResult;

pub(crate) fn weigh_particle<M>(scores: Vec<f64>, model: &M, prev_gen: Generation<M::Parameters>, tolerance: f64) -> Option<Particle<M::Parameters>>
where
    M: Model,
{
    // Get distribution of scores from last generation then reduce by tolerance descent rate (configured) - crate exists for percentile => 
    // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
    // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
    //todo!()
    //(1) Calculating fhat
     let number_reps = scores.len();
    let number_reps_less_than_tolerance = 
    scores
    .iter()
    .filter_map(|score| 
        if score>&tolerance {  None }
        else {  Some(score) }).collect::<Vec<_>>().len();
    let fhat = number_reps_less_than_tolerance/number_reps;

    if fhat == 0 { None }
    else {
       todo!()
    }
}

//TODO add a type alias in where clause for M::Parameters?
pub(crate) fn normalise<M: Model>(
    particles: Vec<Particle<M::Parameters>>,
    gen_num: u16,
) -> Generation<M::Parameters> {
    todo!()
}
