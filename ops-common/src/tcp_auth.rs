use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<Sha256>;

/// TCP认证消息类型
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "auth_type")]
pub enum TcpAuthMessage {
    /// 服务器发送给客户端的认证质询
    #[serde(rename = "challenge")]
    Challenge {
        nonce: String,
        timestamp: u64,
    },
    
    /// 客户端发送给服务器的认证响应
    #[serde(rename = "response")]
    Response {
        client_id: String,
        nonce: String,
        response_hash: String,
        timestamp: u64,
    },
    
    /// 服务器发送给客户端的认证结果
    #[serde(rename = "result")]
    AuthResult {
        success: bool,
        message: String,
    },
}

/// TCP认证器
#[derive(Clone)]
pub struct TcpAuthenticator {
    shared_secret: String,
}

impl TcpAuthenticator {
    /// 创建新的TCP认证器
    pub fn new(shared_secret: String) -> Self {
        Self { shared_secret }
    }
    
    /// 生成认证质询
    pub fn generate_challenge() -> TcpAuthMessage {
        let nonce = uuid::Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        TcpAuthMessage::Challenge { nonce, timestamp }
    }
    
    /// 生成客户端认证响应
    pub fn generate_response(&self, client_id: String, challenge_nonce: String, challenge_timestamp: u64) -> Result<TcpAuthMessage, Box<dyn std::error::Error + Send + Sync>> {
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // 检查时间戳是否在合理范围内（30秒内）
        if current_timestamp.saturating_sub(challenge_timestamp) > 30 {
            return Err("Challenge timestamp too old".into());
        }
        
        // 计算响应哈希: HMAC-SHA256(shared_secret, client_id + nonce + timestamp)
        let data = format!("{}{}{}", client_id, challenge_nonce, challenge_timestamp);
        let response_hash = self.compute_hmac(&data)?;
        
        Ok(TcpAuthMessage::Response {
            client_id,
            nonce: challenge_nonce,
            response_hash,
            timestamp: current_timestamp,
        })
    }
    
    /// 验证客户端响应
    pub fn verify_response(&self, response: &TcpAuthMessage, original_nonce: &str, original_timestamp: u64) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if let TcpAuthMessage::Response { client_id, nonce, response_hash, timestamp } = response {
            // 验证nonce匹配
            if nonce != original_nonce {
                return Ok(false);
            }
            
            // 验证时间戳在合理范围内（60秒内）
            let current_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            if current_timestamp.saturating_sub(*timestamp) > 60 {
                return Ok(false);
            }
            
            // 重新计算期望的响应哈希
            let data = format!("{}{}{}", client_id, original_nonce, original_timestamp);
            let expected_hash = self.compute_hmac(&data)?;
            
            // 使用恒定时间比较防止时序攻击
            Ok(constant_time_compare(&expected_hash, response_hash))
        } else {
            Ok(false)
        }
    }
    
    /// 计算HMAC-SHA256
    fn compute_hmac(&self, data: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut mac = HmacSha256::new_from_slice(self.shared_secret.as_bytes())?;
        mac.update(data.as_bytes());
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }
    
    /// 创建认证成功消息
    pub fn create_success_result() -> TcpAuthMessage {
        TcpAuthMessage::AuthResult {
            success: true,
            message: "Authentication successful".to_string(),
        }
    }
    
    /// 创建认证失败消息
    pub fn create_failure_result(message: &str) -> TcpAuthMessage {
        TcpAuthMessage::AuthResult {
            success: false,
            message: message.to_string(),
        }
    }
}

/// 恒定时间字符串比较，防止时序攻击
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (byte_a, byte_b) in a.bytes().zip(b.bytes()) {
        result |= byte_a ^ byte_b;
    }
    
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tcp_authentication_flow() {
        let shared_secret = "test-secret-key-123";
        let server_auth = TcpAuthenticator::new(shared_secret.to_string());
        let client_auth = TcpAuthenticator::new(shared_secret.to_string());
        
        // 1. 服务器生成质询
        let challenge = TcpAuthenticator::generate_challenge();
        if let TcpAuthMessage::Challenge { nonce, timestamp } = &challenge {
            // 2. 客户端生成响应
            let response = client_auth.generate_response(
                "test-client-id".to_string(),
                nonce.clone(),
                *timestamp
            ).unwrap();
            
            // 3. 服务器验证响应
            let is_valid = server_auth.verify_response(&response, nonce, *timestamp).unwrap();
            assert!(is_valid, "Authentication should succeed with correct credentials");
        } else {
            panic!("Challenge message should be of Challenge type");
        }
    }
    
    #[test]
    fn test_authentication_with_wrong_secret() {
        let server_auth = TcpAuthenticator::new("server-secret".to_string());
        let client_auth = TcpAuthenticator::new("wrong-secret".to_string());
        
        let challenge = TcpAuthenticator::generate_challenge();
        if let TcpAuthMessage::Challenge { nonce, timestamp } = &challenge {
            let response = client_auth.generate_response(
                "test-client-id".to_string(),
                nonce.clone(),
                *timestamp
            ).unwrap();
            
            let is_valid = server_auth.verify_response(&response, nonce, *timestamp).unwrap();
            assert!(!is_valid, "Authentication should fail with wrong secret");
        }
    }
    
    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("hello", "hello"));
        assert!(!constant_time_compare("hello", "world"));
        assert!(!constant_time_compare("hello", "hello!"));
        assert!(!constant_time_compare("", "hello"));
    }
}