use super::base::BaseApi;
use crate::model::product::ProductRepo;

pub struct ProductApi;
impl BaseApi<ProductRepo> for ProductApi {}
