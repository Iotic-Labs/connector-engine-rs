use iotics_grpc_client::GeoLocation;

#[derive(Debug, Clone)]
pub struct Twin {
    pub model_did: String,
    pub seed: String,
    pub label: String,
    pub location: Option<GeoLocation>,
}

impl Twin {
    pub fn new(
        model_did: String,
        seed: String,
        label: String,
        location: Option<GeoLocation>,
    ) -> Self {
        Self {
            model_did,
            seed,
            label,
            location,
        }
    }
}
