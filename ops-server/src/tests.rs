#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_data_handle::{SharedDataHandle, SharedData};
    use crate::middleware::AuthConfig;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use axum_test::TestServer;
    use ops_common::config::ServerConfig;
    use serde_json::json;

    fn create_test_shared_data() -> SharedDataHandle {
        SharedDataHandle::new(SharedData::new(100))
    }

    #[tokio::test]
    async fn test_health_check() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(None);
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/health").await;
        
        response.assert_status(StatusCode::OK);
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["clients_count"], 0);
    }

    #[tokio::test]
    async fn test_auth_middleware_without_token() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(None); // 认证未启用
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/clients").await;
        response.assert_status(StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_middleware_with_valid_token() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(Some("test-token".to_string()));
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/api/clients")
            .add_header("Authorization", "Bearer test-token")
            .await;
        
        response.assert_status(StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_middleware_with_invalid_token() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(Some("test-token".to_string()));
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/api/clients")
            .add_header("Authorization", "Bearer wrong-token")
            .await;
        
        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_missing_header() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(Some("test-token".to_string()));
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/clients").await;
        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(None);
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let payload = json!({
            "message": "Test broadcast message"
        });

        let response = server
            .post("/api/send-message")
            .json(&payload)
            .await;

        response.assert_status(StatusCode::OK);
        let body = response.text();
        assert!(body.contains("消息已广播"));
    }

    #[tokio::test]
    async fn test_send_command() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(None);
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let payload = json!({
            "client_id": "test-client",
            "command": "echo hello"
        });

        let response = server
            .post("/api/send-command")
            .json(&payload)
            .await;

        // 应该返回错误，因为客户端不存在
        response.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let shared_data = create_test_shared_data();
        let auth_config = AuthConfig::new(None);
        let app = crate::web::routes::routes(shared_data, auth_config);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/health").await;
        
        response.assert_status(StatusCode::OK);
        assert!(response.headers().get("access-control-allow-origin").is_some());
    }
}