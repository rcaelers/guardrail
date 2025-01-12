use entities::{
    entity::{attachment, prelude::Attachment},
    model::attachment::{AttachmentCreateDto, AttachmentUpdateDto},
};

use super::base::{NoneFilter, Resource};

impl Resource for Attachment {
    type Entity = attachment::Entity;
    type ActiveModel = attachment::ActiveModel;
    type Data = attachment::Model;
    type CreateData = AttachmentCreateDto;
    type UpdateData = AttachmentUpdateDto;
    type Filter = NoneFilter;
}

#[cfg(test)]
mod tests {
    use crate::api::base::tests::*;
    use axum_test::TestServer;
    use entities::entity::attachment;
    use serial_test::serial;

    #[derive(serde::Deserialize, Debug)]
    struct ApiResponseWithPayload {
        pub result: String,
        pub payload: attachment::Model,
    }

    #[derive(serde::Deserialize, Debug)]
    struct ApiResponseWithVecPayload {
        pub result: String,
        pub payload: Vec<attachment::Model>,
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
                    "report":"Report1", "version": "1.11", "product": "Workrave","summary": "Summary1"
                }))
                .await;
            response.assert_status_ok();
            let crash1 = response.json::<ApiResponseWithId>();
            assert_eq!(crash1.result, "ok");

            let response = server
                .post("/api/crash")
                .content_type("application/json")
                .json(&serde_json::json!({
                    "report":"Report2", "version": "1.11", "product": "Workrave","summary": "Summary1"
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
    async fn test_add_attachment() {
        let context = Context::new().await;

        let response = context
            .server
            .post("/api/attachment")
            .content_type("application/json")
            .json(&serde_json::json!({
              "name": "name1", "mime_type": "mimetype1", "size": 123, "filename": "filename1", "crash_id": format!("{}", context.crash1.id)
            }))
            .await;
        response.assert_status_ok();
        let attachment1 = response.json::<ApiResponseWithId>();
        assert_eq!(attachment1.result, "ok");

        let response = context
            .server
            .post("/api/attachment")
            .content_type("application/json")
            .json(&serde_json::json!({
              "name": "name2", "mime_type": "mimetype2", "size": 123, "filename": "filename2", "crash_id": format!("{}", context.crash1.id)
            }))
            .await;
        response.assert_status_ok();
        let attachment2 = response.json::<ApiResponseWithId>();
        assert_eq!(attachment2.result, "ok");

        let response = context
            .server
            .get("/api/attachment")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let attachments = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(attachments.result, "ok");
        assert_eq!(attachments.payload.len(), 2);
        assert_eq!(attachments.payload[0].id.to_string(), attachment1.id);
        assert_eq!(attachments.payload[1].id.to_string(), attachment2.id);
        assert_eq!(attachments.payload[0].name, "name1");
        assert_eq!(attachments.payload[1].name, "name2");
        assert_eq!(attachments.payload[0].mime_type, "mimetype1");
        assert_eq!(attachments.payload[1].mime_type, "mimetype2");
        assert_eq!(attachments.payload[0].size, 123);
        assert_eq!(attachments.payload[1].size, 123);
        assert_eq!(attachments.payload[0].filename, "filename1");
        assert_eq!(attachments.payload[1].filename, "filename2");

        assert_eq!(
            attachments.payload[0].crash_id.to_string(),
            context.crash1.id
        );
        assert_eq!(
            attachments.payload[1].crash_id.to_string(),
            context.crash1.id
        );

        let response = context
            .server
            .get(format!("/api/attachment/{}", attachment1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let mut attachment = response.json::<ApiResponseWithPayload>();
        assert_eq!(attachment.result, "ok");
        assert_eq!(attachment.payload.id.to_string(), attachment1.id);
        assert_eq!(attachment.payload.name, "name1");
        assert_eq!(attachment.payload.mime_type, "mimetype1");
        assert_eq!(attachment.payload.size, 123);
        assert_eq!(attachment.payload.filename, "filename1");
        assert_eq!(attachment.payload.crash_id.to_string(), context.crash1.id);

        attachment.payload.name = "name1a".to_string();
        attachment.payload.filename = "filename1a".to_string();

        let response = context
            .server
            .put(format!("/api/attachment/{}", attachment1.id).as_str())
            .content_type("application/json")
            .json(&serde_json::json!(attachment.payload))
            .await;
        response.assert_status_ok();
        let attachment = response.json::<ApiResponse>();
        assert_eq!(attachment.result, "ok");

        let response = context
            .server
            .get(format!("/api/attachment/{}", attachment1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let attachment = response.json::<ApiResponseWithPayload>();
        assert_eq!(attachment.result, "ok");
        assert_eq!(attachment.payload.id.to_string(), attachment1.id);
        assert_eq!(attachment.payload.name, "name1a");
        assert_eq!(attachment.payload.mime_type, "mimetype1");
        assert_eq!(attachment.payload.size, 123);
        assert_eq!(attachment.payload.filename, "filename1a");
        assert_eq!(attachment.payload.crash_id.to_string(), context.crash1.id);

        let response = context
            .server
            .delete(format!("/api/attachment/{}", attachment2.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let attachment = response.json::<ApiResponse>();
        assert_eq!(attachment.result, "ok");

        let response = context
            .server
            .get("/api/attachment")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let attachments = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(attachments.result, "ok");
        assert_eq!(attachments.payload.len(), 1);
        assert_eq!(attachments.payload[0].id.to_string(), attachment1.id);
        assert_eq!(attachments.payload[0].name, "name1a");
        assert_eq!(attachments.payload[0].mime_type, "mimetype1");
        assert_eq!(attachments.payload[0].size, 123);
        assert_eq!(attachments.payload[0].filename, "filename1a");
        assert_eq!(
            attachments.payload[0].crash_id.to_string(),
            context.crash1.id
        );
    }

    #[serial]
    #[tokio::test]
    async fn test_incomplete_json() {
        let context = Context::new().await;

        let response = context
            .server
            .post("/api/attachment")
            .content_type("application/json")
            .json(&serde_json::json!({}))
            .await;

        response.assert_status_bad_request();
        let attachment = response.json::<ApiResponseFailed>();
        assert_eq!(attachment.result, "failed");
    }
}
