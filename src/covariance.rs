

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use crate::Generation;

    #[derive(Deserialize, Debug)]
    struct Params{
        x: f64,
        y:f64,
    }

    #[test]
    fn load_generation(){
        let path = "resources/test/covariance/particles.json";
        let generation: Generation<Params> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        println!("Generation = {:#?}", generation);
    }


}