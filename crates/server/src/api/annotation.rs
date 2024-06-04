use crate::{
    entity::{annotation, prelude::Annotation},
    model::annotation::{AnnotationCreateDto, AnnotationUpdateDto},
};

use super::base::{NoneFilter, Resource};

impl Resource for Annotation {
    type Entity = annotation::Entity;
    type ActiveModel = annotation::ActiveModel;
    type Data = annotation::Model;
    type CreateData = AnnotationCreateDto;
    type UpdateData = AnnotationUpdateDto;
    type Filter = NoneFilter;
}

#[cfg(test)]
mod tests {
    use crate::entity::annotation;
    use crate::{api::base::tests::*, entity::sea_orm_active_enums::AnnotationKind};
    use axum_test::TestServer;
    use serial_test::serial;

    #[derive(serde::Deserialize, Debug)]
    struct ApiResponseWithPayload {
        pub result: String,
        pub payload: annotation::Model,
    }

    #[derive(serde::Deserialize, Debug)]
    struct ApiResponseWithVecPayload {
        pub result: String,
        pub payload: Vec<annotation::Model>,
    }

    struct Context {
        pub server: TestServer,
        pub product1: ApiResponseWithId,
        pub product2: ApiResponseWithId,
        pub version1: ApiResponseWithId,
        pub version2: ApiResponseWithId,
        pub crash1: ApiResponseWithId,
        pub crash2: ApiResponseWithId,
    }

    impl Context {
        pub async fn new() -> Context {
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

            let response = server
                .post("/api/product")
                .content_type("application/json")
                .json(&serde_json::json!({
                  "name":"Scroom"
                }))
                .await;
            response.assert_status_ok();
            let product2 = response.json::<ApiResponseWithId>();

            let response = server
                .post("/api/version")
                .content_type("application/json")
                .json(&serde_json::json!({
                    "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave"
                }))
                .await;
            response.assert_status_ok();
            let version1 = response.json::<ApiResponseWithId>();
            assert_eq!(version1.result, "ok");

            let response = server
              .post("/api/version")
              .content_type("application/json")
              .json(&serde_json::json!({
                  "name":"1.12", "hash":"1234567890", "tag": "v1.12", "product_id": format!("{}", product1.id)
              }))
              .await;
            response.assert_status_ok();
            let version2 = response.json::<ApiResponseWithId>();
            assert_eq!(version2.result, "ok");

            let response = server
                .post("/api/crash")
                .content_type("application/json")
                .json(&serde_json::json!({
                    "report":"Report1", "version": "1.11", "product": "Workrave"
                }))
                .await;
            response.assert_status_ok();
            let crash1 = response.json::<ApiResponseWithId>();
            assert_eq!(crash1.result, "ok");

            let response = server
                .post("/api/crash")
                .content_type("application/json")
                .json(&serde_json::json!({
                    "report":"Report2", "version": "1.11", "product": "Workrave"
                }))
                .await;
            response.assert_status_ok();
            let crash2 = response.json::<ApiResponseWithId>();
            assert_eq!(crash2.result, "ok");

            Context {
                server,
                product1,
                product2,
                version1,
                version2,
                crash1,
                crash2,
            }
        }
    }

    #[serial]
    #[tokio::test]
    async fn test_add_annotation() {
        let context = Context::new().await;

        let response = context
            .server
            .post("/api/annotation")
            .content_type("application/json")
            .json(&serde_json::json!({
               "key": "key1",  "kind": "System", "value": "value1", "crash_id": format!("{}", context.crash1.id)
            }))
            .await;
        response.assert_status_ok();
        let annotation1 = response.json::<ApiResponseWithId>();
        assert_eq!(annotation1.result, "ok");

        let response = context
            .server
            .post("/api/annotation")
            .content_type("application/json")
            .json(&serde_json::json!({
              "key": "key2",  "kind": "User", "value": "value2", "crash_id": format!("{}", context.crash1.id)
            }))
            .await;
        response.assert_status_ok();
        let annotation2 = response.json::<ApiResponseWithId>();
        assert_eq!(annotation2.result, "ok");

        let response = context
            .server
            .get("/api/annotation")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let annotations = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(annotations.result, "ok");
        assert_eq!(annotations.payload.len(), 2);
        assert_eq!(annotations.payload[0].id.to_string(), annotation1.id);
        assert_eq!(annotations.payload[1].id.to_string(), annotation2.id);
        assert_eq!(annotations.payload[0].key, "key1");
        assert_eq!(annotations.payload[1].key, "key2");
        assert_eq!(annotations.payload[0].value, "value1");
        assert_eq!(annotations.payload[1].value, "value2");
        assert_eq!(annotations.payload[0].kind, AnnotationKind::System);
        assert_eq!(annotations.payload[1].kind, AnnotationKind::User);
        assert_eq!(
            annotations.payload[0].crash_id.to_string(),
            context.crash1.id
        );
        assert_eq!(
            annotations.payload[1].crash_id.to_string(),
            context.crash1.id
        );

        let response = context
            .server
            .get(format!("/api/annotation/{}", annotation1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let mut annotation = response.json::<ApiResponseWithPayload>();
        assert_eq!(annotation.result, "ok");
        assert_eq!(annotation.payload.id.to_string(), annotation1.id);
        assert_eq!(annotation.payload.key, "key1");
        assert_eq!(annotation.payload.value, "value1");
        assert_eq!(annotation.payload.kind, AnnotationKind::System);

        annotation.payload.key = "key1a".to_string();
        annotation.payload.value = "value1a".to_string();

        let response = context
            .server
            .put(format!("/api/annotation/{}", annotation1.id).as_str())
            .content_type("application/json")
            .json(&serde_json::json!(annotation.payload))
            .await;
        response.assert_status_ok();
        let annotation = response.json::<ApiResponse>();
        assert_eq!(annotation.result, "ok");

        let response = context
            .server
            .get(format!("/api/annotation/{}", annotation1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let annotation = response.json::<ApiResponseWithPayload>();
        assert_eq!(annotation.result, "ok");
        assert_eq!(annotation.payload.id.to_string(), annotation1.id);
        assert_eq!(annotation.payload.key, "key1a");
        assert_eq!(annotation.payload.value, "value1a");
        assert_eq!(annotation.payload.kind, AnnotationKind::System);

        let response = context
            .server
            .delete(format!("/api/annotation/{}", annotation2.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let annotation = response.json::<ApiResponse>();
        assert_eq!(annotation.result, "ok");

        let response = context
            .server
            .get("/api/annotation")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let annotations = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(annotations.result, "ok");
        assert_eq!(annotations.payload.len(), 1);
        assert_eq!(annotations.payload[0].id.to_string(), annotation1.id);
        assert_eq!(annotations.payload[0].key, "key1a");
        assert_eq!(annotations.payload[0].value, "value1a");
        assert_eq!(annotations.payload[0].kind, AnnotationKind::System);
    }

    #[serial]
    #[tokio::test]
    async fn test_incomplete_json() {
        let context = Context::new().await;

        let response = context
            .server
            .post("/api/annotation")
            .content_type("application/json")
            .json(&serde_json::json!({}))
            .await;

        response.assert_status_bad_request();
        let annotation = response.json::<ApiResponseFailed>();
        assert_eq!(annotation.result, "failed");
    }
}
