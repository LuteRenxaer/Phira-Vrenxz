use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanInfo {
    pub user_id: Option<i32>, // 可选的用户ID（如果为IP封禁则为None）
    pub user_name: Option<String>, // 可选的用户名
    pub ip_address: Option<String>, // 可选的IP地址（如果为ID封禁则为None）
    pub ban_reason: String,
    pub ban_start: u64,  // Unix时间戳
    pub ban_duration: u64, // 封禁持续时间（秒）
    pub banned_by: String, // 封禁操作者
    pub ban_type: BanType, // 封禁类型
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BanType {
    UserId,  // 用户ID封禁
    Ip,      // IP地址封禁
    UserIdAndIp, // 用户ID和IP地址同时封禁
}

impl BanInfo {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        
        now >= self.ban_start + self.ban_duration
    }
    
    pub fn remaining_time(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        
        if now >= self.ban_start + self.ban_duration {
            0
        } else {
            (self.ban_start + self.ban_duration) - now
        }
    }
    
    pub fn ban_end_time(&self) -> u64 {
        self.ban_start + self.ban_duration
    }
    
    pub fn matches_user(&self, user_id: i32) -> bool {
        match self.ban_type {
            BanType::UserId => self.user_id.map_or(false, |id| id == user_id),
            BanType::Ip => false, // IP封禁不匹配用户ID
            BanType::UserIdAndIp => self.user_id.map_or(false, |id| id == user_id),
        }
    }
    
    pub fn matches_ip(&self, ip_address: &str) -> bool {
        match self.ban_type {
            BanType::UserId => false, // ID封禁不匹配IP
            BanType::Ip => self.ip_address.as_deref().map_or(false, |ip| ip == ip_address),
            BanType::UserIdAndIp => self.ip_address.as_deref().map_or(false, |ip| ip == ip_address),
        }
    }
}

#[derive(Clone)]
pub struct BanManager {
    pub banned_items: Arc<RwLock<HashMap<String, BanInfo>>>, // 使用组合键存储封禁信息
}

impl BanManager {
    pub fn new() -> Self {
        Self {
            banned_items: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn load_bans_from_file(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if std::path::Path::new(file_path).exists() {
            let content = tokio::fs::read_to_string(file_path).await?;
            let bans: HashMap<String, BanInfo> = serde_json::from_str(&content)?;
            *self.banned_items.write().await = bans;
        }
        Ok(())
    }
    
    pub async fn save_bans_to_file(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let bans = self.banned_items.read().await;
        let content = serde_json::to_string_pretty(&*bans)?;
        tokio::fs::write(file_path, content).await?;
        Ok(())
    }
    
    pub async fn add_ban(&self, ban_info: BanInfo) -> Result<(), String> {
        let mut bans = self.banned_items.write().await;
        
        // 根据封禁类型生成唯一键
        let key = match ban_info.ban_type {
            BanType::UserId => format!("user_{}", ban_info.user_id.unwrap_or(0)),
            BanType::Ip => format!("ip_{}", ban_info.ip_address.as_deref().unwrap_or("")),
            BanType::UserIdAndIp => format!("userip_{}_{}", 
                ban_info.user_id.unwrap_or(0), 
                ban_info.ip_address.as_deref().unwrap_or("")),
        };
        
        bans.insert(key, ban_info);
        Ok(())
    }
    
    pub async fn remove_ban(&self, key: &str) -> Result<(), String> {
        let mut bans = self.banned_items.write().await;
        match bans.remove(key) {
            Some(_) => Ok(()),
            None => Err("未找到对应的封禁记录".to_string()),
        }
    }
    
    pub async fn is_user_banned(&self, user_id: i32) -> Option<BanInfo> {
        let bans = self.banned_items.read().await;
        for (_, ban_info) in bans.iter() {
            if !ban_info.is_expired() && ban_info.matches_user(user_id) {
                return Some(ban_info.clone());
            }
        }
        None
    }
    
    pub async fn is_ip_banned(&self, ip_address: &str) -> Option<BanInfo> {
        let bans = self.banned_items.read().await;
        for (_, ban_info) in bans.iter() {
            if !ban_info.is_expired() && ban_info.matches_ip(ip_address) {
                return Some(ban_info.clone());
            }
        }
        None
    }
    
    pub async fn is_user_or_ip_banned(&self, user_id: i32, ip_address: &str) -> Option<BanInfo> {
        // 首先检查用户ID
        if let Some(ban_info) = self.is_user_banned(user_id).await {
            return Some(ban_info);
        }
        
        // 然后检查IP地址
        if let Some(ban_info) = self.is_ip_banned(ip_address).await {
            return Some(ban_info);
        }
        
        None
    }
    
    pub async fn get_all_bans(&self) -> Vec<BanInfo> {
        let bans = self.banned_items.read().await;
        bans.values()
            .filter(|ban| !ban.is_expired())
            .cloned()
            .collect()
    }
    
    pub async fn cleanup_expired_bans(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut bans = self.banned_items.write().await;
        bans.retain(|_, ban_info| !ban_info.is_expired());
        Ok(())
    }
}