#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;
    use ops_common::{
        config::ClientConfig,
        security::{CommandValidator, ValidationResult}
    };

    #[test]
    fn test_client_config_from_env() {
        std::env::set_var("OPS_SERVER_HOST", "test-server");
        std::env::set_var("OPS_SERVER_PORT", "9999");
        
        let config = ClientConfig::from_env();
        assert_eq!(config.server_host, "test-server");
        assert_eq!(config.server_port, 9999);
        
        // 清理环境变量
        std::env::remove_var("OPS_SERVER_HOST");
        std::env::remove_var("OPS_SERVER_PORT");
    }

    #[test]
    fn test_client_id_creation_and_retrieval() {
        let temp_dir = tempdir().unwrap();
        let client_id_file = temp_dir.path().join("test_client_id.txt");
        
        let mut config = ClientConfig::default();
        config.client_id_file = client_id_file.to_str().unwrap().to_string();

        // 测试创建新的客户端ID
        let id1 = crate::tcp_services::client::get_or_create_client_id(&config).unwrap();
        assert!(!id1.is_empty());
        assert!(client_id_file.exists());

        // 测试读取现有的客户端ID
        let id2 = crate::tcp_services::client::get_or_create_client_id(&config).unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_command_validator_allowed_commands() {
        let validator = CommandValidator::new();
        
        match validator.validate("ps aux") {
            ValidationResult::Allowed => {},
            ValidationResult::Blocked { reason } => panic!("Should be allowed: {}", reason),
        }

        match validator.validate("ls -la") {
            ValidationResult::Allowed => {},
            ValidationResult::Blocked { reason } => panic!("Should be allowed: {}", reason),
        }
    }

    #[test]
    fn test_command_validator_dangerous_commands() {
        let validator = CommandValidator::new();
        
        match validator.validate("rm -rf /") {
            ValidationResult::Blocked { reason } => {
                assert!(reason.contains("危险模式") || reason.contains("dangerous"));
            },
            ValidationResult::Allowed => panic!("Should be blocked"),
        }

        match validator.validate("shutdown now") {
            ValidationResult::Blocked { reason } => {
                assert!(reason.contains("危险模式") || reason.contains("dangerous"));
            },
            ValidationResult::Allowed => panic!("Should be blocked"),
        }
    }

    #[test]
    fn test_command_validator_unknown_commands() {
        let validator = CommandValidator::new();
        
        match validator.validate("malicious_unknown_command") {
            ValidationResult::Blocked { reason } => {
                assert!(reason.contains("不在允许列表中") || reason.contains("not allowed"));
            },
            ValidationResult::Allowed => panic!("Should be blocked"),
        }
    }

    #[test]
    fn test_command_validator_empty_command() {
        let validator = CommandValidator::new();
        
        match validator.validate("") {
            ValidationResult::Blocked { .. } => {},
            ValidationResult::Allowed => panic!("Empty command should be blocked"),
        }

        match validator.validate("   ") {
            ValidationResult::Blocked { .. } => {},
            ValidationResult::Allowed => panic!("Whitespace-only command should be blocked"),
        }
    }

    #[test]
    fn test_command_validator_long_command() {
        let validator = CommandValidator::new();
        let long_command = "a".repeat(2000); // 超过默认的1000字符限制
        
        match validator.validate(&long_command) {
            ValidationResult::Blocked { reason } => {
                assert!(reason.contains("长度超过限制") || reason.contains("too long"));
            },
            ValidationResult::Allowed => panic!("Long command should be blocked"),
        }
    }

    #[test]
    fn test_command_sanitization() {
        let validator = CommandValidator::new();
        
        let dirty_command = "ps aux; rm -rf /; echo hello";
        let clean_command = validator.sanitize_command(dirty_command);
        
        // 应该移除危险的分隔符
        assert!(!clean_command.contains(";"));
        assert!(!clean_command.contains("&&"));
        assert!(!clean_command.contains("||"));
    }

    #[test]
    fn test_version_collector_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let versions = crate::collection::version_collector::read_app_versions(
            temp_dir.path().to_str().unwrap()
        );
        assert!(versions.is_empty());
    }

    #[test]
    fn test_version_collector_with_valid_version_file() {
        let temp_dir = tempdir().unwrap();
        let app_dir = temp_dir.path().join("test_app");
        fs::create_dir(&app_dir).unwrap();
        
        let version_file = app_dir.join("version.txt");
        let version_data = r#"{"app":"test_app","created_time":"2024-01-01T00:00:00Z"}"#;
        fs::write(&version_file, version_data).unwrap();
        
        let versions = crate::collection::version_collector::read_app_versions(
            temp_dir.path().to_str().unwrap()
        );
        
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].app, "test_app");
        assert_eq!(versions[0].created_time, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_version_collector_with_invalid_json() {
        let temp_dir = tempdir().unwrap();
        let app_dir = temp_dir.path().join("bad_app");
        fs::create_dir(&app_dir).unwrap();
        
        let version_file = app_dir.join("version.txt");
        fs::write(&version_file, "invalid json").unwrap();
        
        let versions = crate::collection::version_collector::read_app_versions(
            temp_dir.path().to_str().unwrap()
        );
        
        // 应该忽略无效的JSON文件
        assert!(versions.is_empty());
    }
}