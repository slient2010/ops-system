use std::fs;
use std::path::Path;
use ops_common::{AppInfo, ServiceStatus};
use tracing::{debug, warn, error};
use serde_json;

pub struct AppInfoCollector {
    apps_dir: String,
}

impl AppInfoCollector {
    pub fn new(apps_dir: String) -> Self {
        Self { apps_dir }
    }

    /// 读取指定目录下的所有应用信息
    pub fn collect_apps_info(&self) -> Vec<AppInfo> {
        let mut apps = Vec::new();
        
        let apps_path = Path::new(&self.apps_dir);
        if !apps_path.exists() || !apps_path.is_dir() {
            warn!("Apps directory does not exist or is not a directory: {}", self.apps_dir);
            return apps;
        }

        debug!("Reading apps from directory: {}", self.apps_dir);

        match fs::read_dir(apps_path) {
            Ok(entries) => {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Some(app_info) = self.read_app_info(&entry.path()) {
                            apps.push(app_info);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to read apps directory {}: {}", self.apps_dir, e);
            }
        }

        debug!("Collected {} app(s) info", apps.len());
        apps
    }

    /// 读取单个应用的信息
    fn read_app_info(&self, app_path: &Path) -> Option<AppInfo> {
        // 只处理目录
        if !app_path.is_dir() {
            return None;
        }

        let app_name = app_path
            .file_name()?
            .to_str()?
            .to_string();

        let version_file = app_path.join("version.txt");
        if !version_file.exists() {
            debug!("No version.txt found for app: {}", app_name);
            return None;
        }

        match fs::read_to_string(&version_file) {
            Ok(content) => {
                let app_info = self.parse_version_content(&app_name, &content);
                debug!("Successfully read app info: {}", app_name);
                Some(app_info)
            }
            Err(e) => {
                warn!("Failed to read version file for {}: {}", app_name, e);
                None
            }
        }
    }

    /// 解析version.txt文件内容 (支持JSON和原有key:value格式)
    fn parse_version_content(&self, app_name: &str, content: &str) -> AppInfo {
        let service_status = self.check_service_status(app_name);

        // 首先尝试解析为JSON格式
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(content.trim()) {
            let version = json_value.get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            let deploy_time = json_value.get("deploy_time")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            let branch = json_value.get("branch")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            let commit = json_value.get("commit")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            debug!("Parsed JSON version info for {}: version={}, deploy_time={}", app_name, version, deploy_time);

            return AppInfo {
                name: app_name.to_string(),
                version,
                deploy_time,
                branch,
                commit,
                service_status,
            };
        }

        // 如果JSON解析失败，回退到原有的key:value格式
        debug!("JSON parsing failed for {}, falling back to key:value format", app_name);
        let mut version = "unknown".to_string();
        let mut deploy_time = "unknown".to_string();
        let mut branch = None;
        let mut commit = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "version" => version = value.to_string(),
                    "deploy_time" => deploy_time = value.to_string(),
                    "branch" => branch = Some(value.to_string()),
                    "commit" => commit = Some(value.to_string()),
                    _ => {} // 忽略其他字段
                }
            }
        }

        AppInfo {
            name: app_name.to_string(),
            version,
            deploy_time,
            branch,
            commit,
            service_status,
        }
    }

    /// 检查服务状态（基于PID文件）
    fn check_service_status(&self, app_name: &str) -> ServiceStatus {
        let app_path = Path::new(&self.apps_dir).join(app_name);
        let pid_file = app_path.join(format!("{}.pid", app_name));

        if !pid_file.exists() {
            debug!("PID file not found for {}, service is stopped", app_name);
            return ServiceStatus::Stopped;
        }

        match fs::read_to_string(&pid_file) {
            Ok(content) => {
                let pid_str = content.trim();
                if pid_str.is_empty() {
                    debug!("Empty PID file for {}, service is stopped", app_name);
                    return ServiceStatus::Stopped;
                }

                // 验证PID是否有效
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if self.is_process_running(pid) {
                        debug!("Service {} is running with PID {}", app_name, pid);
                        ServiceStatus::Running(pid_str.to_string())
                    } else {
                        debug!("Process with PID {} is not running, service {} is stopped", pid, app_name);
                        ServiceStatus::Stopped
                    }
                } else {
                    warn!("Invalid PID format in file for {}: {}", app_name, pid_str);
                    ServiceStatus::Unknown
                }
            }
            Err(e) => {
                warn!("Failed to read PID file for {}: {}", app_name, e);
                ServiceStatus::Unknown
            }
        }
    }

    /// 检查进程是否在运行
    fn is_process_running(&self, pid: u32) -> bool {
        // 在Linux系统上，检查/proc/PID目录是否存在
        #[cfg(target_os = "linux")]
        {
            Path::new(&format!("/proc/{}", pid)).exists()
        }
        
        // 在其他系统上，可以使用其他方法
        #[cfg(not(target_os = "linux"))]
        {
            // 简单实现：尝试发送0号信号
            use std::process::Command;
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_collect_apps_info() {
        // 创建临时测试目录
        let temp_dir = TempDir::new().unwrap();
        let apps_dir = temp_dir.path().join("apps");
        fs::create_dir(&apps_dir).unwrap();

        // 创建测试应用
        let app_dir = apps_dir.join("test-app");
        fs::create_dir(&app_dir).unwrap();
        
        let version_content = "version: v1.0.0\ndeploy_time: 2025-08-29 14:30:25\nbranch: main\ncommit: abc123";
        fs::write(app_dir.join("version.txt"), version_content).unwrap();

        // 测试收集功能
        let collector = AppInfoCollector::new(apps_dir.to_string_lossy().to_string());
        let apps = collector.collect_apps_info();

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "test-app");
        assert_eq!(apps[0].version, "v1.0.0");
        assert_eq!(apps[0].deploy_time, "2025-08-29 14:30:25");
        assert_eq!(apps[0].branch, Some("main".to_string()));
        assert_eq!(apps[0].commit, Some("abc123".to_string()));
        // Service should be stopped since no PID file exists
        matches!(apps[0].service_status, ServiceStatus::Stopped);
    }
}