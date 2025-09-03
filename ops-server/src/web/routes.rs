// use axum::{Router, routing::get};

// use crate::{web::handlers, SharedDataHandle};

// pub fn routes(shared_data: SharedDataHandle) -> Router {
//     Router::new()
//         .route("/data", get(handlers::list_clients))
//         .with_state(shared_data)
// }
use axum::{
    Router, 
    routing::{get, post},
    middleware,
};
use crate::{web::handlers, SharedDataHandle, middleware::{auth_middleware, cors_middleware, web_logging_middleware, AuthConfig}};
use crate::web::handlers::SessionStore;

pub fn routes(shared_data: SharedDataHandle, auth_config: AuthConfig) -> (Router, SessionStore) {
    // 创建会话存储
    let session_store = SessionStore::new();

    // 将session_store集成到auth_config中
    let auth_config_with_session = auth_config.with_session_store(session_store.clone());

    // 认证相关的公开路由
    let auth_routes = Router::new()
        .route("/api/login", post(handlers::login))
        .route("/api/logout", post(handlers::logout))
        .route("/api/check-auth", get(handlers::check_auth))
        .with_state(session_store.clone());

    // 其他公开路由
    let public_routes = Router::new()
        .route("/", get(handlers::index))
        .route("/health", get(handlers::health_check))
        .with_state(shared_data.clone());

    // 需要认证的API路由
    let protected_routes = Router::new()
        .route("/api/clients", get(handlers::list_clients))
        .route("/api/send-message", post(handlers::broadcast_message))
        .route("/api/send-command", post(handlers::send_command))
        .route("/api/command-result", get(handlers::get_command_result))
        .route("/api/client-history", get(handlers::get_client_command_history))
        .route("/api/predefined-commands", get(handlers::get_predefined_commands))
        .route("/api/apps", get(handlers::get_apps_info))
        .route("/api/client-apps", get(handlers::get_client_apps_info))
        .route("/api/manage-service", post(handlers::manage_service))
        .route("/api/update-app", post(handlers::update_app))
        .route("/data", get(handlers::list_clients))  // 保持原有路由
        .layer(middleware::from_fn_with_state(auth_config_with_session.clone(), auth_middleware))
        .with_state(shared_data.clone());

    // 组合路由
    let router = Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn(cors_middleware))
        .layer(middleware::from_fn(web_logging_middleware));
    
    (router, session_store)
}