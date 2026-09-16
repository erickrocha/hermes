

pub trait EntityMapper<D,M,AM> {


	fn build_active_model( d: D) -> AM;

	fn build_active_model_vec( domains: Vec<D>) -> Vec<AM> {
		domains.into_iter().map(|d| Self::build_active_model(d)).collect()
	}

	fn from_model(e: M) -> D;

	fn from_active_model(e: AM) -> D;

	fn build_active_models(domains: Vec<D> ) -> Vec<AM> {
		domains.into_iter().map(|d| Self::build_active_model(d)).collect()
	}

	fn from_models(entities: Vec<M>) -> Vec<D> {
		entities.into_iter().map(|e| Self::from_model(e)).collect()
	}

	fn from_active_models(entities: Vec<AM>) -> Vec<D> {
		entities.into_iter().map(|e| Self::from_active_model(e)).collect()
	}

}