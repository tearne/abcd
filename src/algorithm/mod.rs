use crate::{Generation, Model, Particle};
use crate::ABCDResult;

pub(crate) fn weigh_particle<M>(scores: Vec<f64>, model: &M, prev_gen: Generation<M::Parameters>, tolerance: f64) -> Option<Particle<M::Parameters>>
where
    M: Model,
{



       todo!()

}

//TODO add a type alias in where clause for M::Parameters?
pub(crate) fn normalise<M: Model>(
    particles: Vec<Particle<M::Parameters>>,
    gen_num: u16,
) -> Generation<M::Parameters> {
    todo!()
}
