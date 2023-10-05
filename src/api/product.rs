use super::base::BaseApi;
use crate::model::product::ProductRepo;

pub struct ProductApi;
impl BaseApi<ProductRepo> for ProductApi {}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use crate::{api::base::tests::*, model::{product::ProductRepo, base::BaseRepo}};
    
    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponseWithPayload {
        pub result: String,
        pub payload: <ProductRepo as BaseRepo>::Repr,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponseWithVecPayload {
        pub result: String,
        pub payload: Vec<<ProductRepo as BaseRepo>::Repr>,
    }
    
    #[serial]
    #[tokio::test]
    async fn test_add_product() {
        //init_logging().await;
        let server = run_server().await;

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
               "name":"Workrave"
            }))
            .await;
        response.assert_status_ok();
        let product1 = response.json::<ApiResponseWithId>();
        assert_eq!(product1.result, "ok");

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
              "name":"Scroom"
            }))
            .await;
        response.assert_status_ok();
        let product2 = response.json::<ApiResponseWithId>();
        assert_eq!(product2.result, "ok");

        let response = server
            .get("/api/product")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let products = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(products.result, "ok");
        assert_eq!(products.payload.len(), 2);
        assert_eq!(products.payload[0].name, "Workrave");
        assert_eq!(products.payload[1].name, "Scroom");
        assert_eq!(products.payload[0].id.to_string(), product1.id);
        assert_eq!(products.payload[1].id.to_string(), product2.id);

        let response = server
            .get(format!("/api/product/{}", product1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let mut product = response.json::<ApiResponseWithPayload>();
        assert_eq!(product.result, "ok");
        assert_eq!(product.payload.name, "Workrave");
        assert_eq!(product.payload.id.to_string(), product1.id);

        product.payload.name = "workrave".to_string();

        let response = server
            .put(format!("/api/product/{}", product1.id).as_str())
            .content_type("application/json")
            .json(&serde_json::json!(product.payload))
            .await;
        response.assert_status_ok();
        let product = response.json::<ApiResponse>();
        assert_eq!(product.result, "ok");

        let response = server
            .get(format!("/api/product/{}", product1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let product = response.json::<ApiResponseWithPayload>();
        assert_eq!(product.result, "ok");
        assert_eq!(product.payload.name, "workrave");
        assert_eq!(product.payload.id.to_string(), product1.id);

        let response = server
            .delete(format!("/api/product/{}", product2.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let product = response.json::<ApiResponse>();
        assert_eq!(product.result, "ok");

        let response = server
            .get("/api/product")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let products = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(products.result, "ok");
        assert_eq!(products.payload.len(), 1);
        assert_eq!(products.payload[0].id.to_string(), product1.id);
    }

    #[serial]
    #[tokio::test]
    async fn test_not_unique() {
        //init_logging().await;
        let server = run_server().await;

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name": "Workrave"
            }))
            .await;
        println!("{:?}", response);
        response.assert_status_ok();
        let product = response.json::<ApiResponse>();
        println!("{:?}", product);
        assert_eq!(product.result, "ok");

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name": "Workrave"
            }))
            .await;
        println!("{:?}", response);

        response.assert_status_bad_request();
        let product = response.json::<ApiResponseFailed>();
        println!("{:?}", product);
        assert_eq!(product.result, "failed");
    }

    #[serial]
    #[tokio::test]
    async fn test_incomplete_json() {
        //init_logging().await;
        let server = run_server().await;

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({}))
            .await;
        println!("{:?}", response);

        response.assert_status_bad_request();
        let product = response.json::<ApiResponseFailed>();
        println!("{:?}", product);
        assert_eq!(product.result, "failed");
    }
}
