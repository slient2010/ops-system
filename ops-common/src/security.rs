use std::collections::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredefinedCommand {
    pub command: String,
    pub name: String,
    pub category: String,
    pub description: String,
}

impl PredefinedCommand {
    pub fn new(command: &str, name: &str, category: &str, description: &str) -> Self {
        Self {
            command: command.to_string(),
            name: name.to_string(),
            category: category.to_string(),
            description: description.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandValidator {
    allowed_commands: HashSet<String>,
    blocked_patterns: Vec<String>,
    max_command_length: usize,
    allowed_script_dirs: Vec<String>, // 允许执行脚本的目录白名单
    allowed_script_extensions: HashSet<String>, // 允许的脚本文件扩展名
}

impl Default for CommandValidator {
    fn default() -> Self {
        let mut allowed_commands = HashSet::new();
        
        // 系统信息查看命令
        allowed_commands.insert("ps".to_string());
        allowed_commands.insert("ls".to_string());
        allowed_commands.insert("pwd".to_string());
        allowed_commands.insert("whoami".to_string());
        allowed_commands.insert("id".to_string());
        allowed_commands.insert("groups".to_string());
        allowed_commands.insert("date".to_string());
        allowed_commands.insert("uptime".to_string());
        allowed_commands.insert("hostname".to_string());
        allowed_commands.insert("uname".to_string());
        
        // 资源监控命令
        allowed_commands.insert("df".to_string());
        allowed_commands.insert("free".to_string());
        allowed_commands.insert("top".to_string());
        allowed_commands.insert("htop".to_string());
        allowed_commands.insert("iostat".to_string());
        allowed_commands.insert("vmstat".to_string());
        allowed_commands.insert("sar".to_string());
        allowed_commands.insert("mpstat".to_string());
        
        // 网络信息查看
        allowed_commands.insert("netstat".to_string());
        allowed_commands.insert("ss".to_string());
        allowed_commands.insert("ip".to_string());
        allowed_commands.insert("ifconfig".to_string());
        allowed_commands.insert("ping".to_string());
        
        // 文件查看命令（只读）
        allowed_commands.insert("cat".to_string());
        allowed_commands.insert("head".to_string());
        allowed_commands.insert("tail".to_string());
        allowed_commands.insert("less".to_string());
        allowed_commands.insert("more".to_string());
        allowed_commands.insert("grep".to_string());
        allowed_commands.insert("find".to_string());
        allowed_commands.insert("wc".to_string());
        allowed_commands.insert("sort".to_string());
        allowed_commands.insert("uniq".to_string());
        
        // 服务管理（只读操作）
        allowed_commands.insert("systemctl".to_string());
        allowed_commands.insert("journalctl".to_string());
        allowed_commands.insert("service".to_string());
        
        // 环境变量和历史
        allowed_commands.insert("env".to_string());
        allowed_commands.insert("history".to_string());
        allowed_commands.insert("which".to_string());
        allowed_commands.insert("whereis".to_string());
        
        // Shell命令（用于执行脚本）
        allowed_commands.insert("bash".to_string());
        allowed_commands.insert("sh".to_string());
        
        // 进程管理命令（用于服务管理）
        allowed_commands.insert("kill".to_string());
        allowed_commands.insert("cd".to_string());

        let blocked_patterns = vec![
            // 系统关机/重启命令
            "shutdown".to_string(),
            "reboot".to_string(),
            "halt".to_string(),
            "poweroff".to_string(),
            "init 0".to_string(),
            "init 6".to_string(),
            "systemctl poweroff".to_string(),
            "systemctl reboot".to_string(),
            "systemctl halt".to_string(),
            
            // 危险的文件操作
            "rm -rf".to_string(),
            "rm -r".to_string(),
            "rm /".to_string(),
            "rmdir".to_string(),
            "> /dev/".to_string(),
            "dd if=".to_string(),
            "dd of=".to_string(),
            "mkfs".to_string(),
            "fdisk".to_string(),
            "parted".to_string(),
            "format".to_string(),
            
            // 权限提升命令
            "sudo su".to_string(),
            "su -".to_string(),
            "su root".to_string(),
            "sudo -i".to_string(),
            "sudo bash".to_string(),
            "sudo sh".to_string(),
            "passwd".to_string(),
            "usermod".to_string(),
            "useradd".to_string(),
            "userdel".to_string(),
            "chmod 777".to_string(),
            "chmod 4755".to_string(),
            "chown root".to_string(),
            
            // 网络下载和连接
            "curl".to_string(),
            "wget".to_string(),
            "nc -".to_string(),
            "netcat".to_string(),
            "telnet".to_string(),
            "ftp".to_string(),
            "sftp".to_string(),
            "scp".to_string(),
            "rsync".to_string(),
            
            // 危险的Shell执行模式
            "bash -i".to_string(),
            "sh -i".to_string(),
            "exec".to_string(),
            "eval".to_string(),
            "source".to_string(),
            "python -c".to_string(),
            "perl -e".to_string(),
            "ruby -e".to_string(),
            
            // 进程和服务控制
            "kill -9".to_string(),
            "killall".to_string(),
            "pkill".to_string(),
            "systemctl start".to_string(),
            "systemctl stop".to_string(),
            "systemctl restart".to_string(),
            "systemctl enable".to_string(),
            "systemctl disable".to_string(),
            "service start".to_string(),
            "service stop".to_string(),
            "service restart".to_string(),
            
            // 包管理器
            "apt install".to_string(),
            "apt remove".to_string(),
            "apt purge".to_string(),
            "yum install".to_string(),
            "yum remove".to_string(),
            "dnf install".to_string(),
            "dnf remove".to_string(),
            "rpm -i".to_string(),
            "rpm -e".to_string(),
            "dpkg -i".to_string(),
            "dpkg -r".to_string(),
            "pip install".to_string(),
            "npm install".to_string(),
            
            // 计划任务和定时任务
            "crontab".to_string(),
            "at ".to_string(),
            "batch".to_string(),
            
            // 挂载和存储操作
            "mount".to_string(),
            "umount".to_string(),
            "fsck".to_string(),
            "e2fsck".to_string(),
            
            // 命令注入防护（只阻止明显的注入模式）
            "`".to_string(),
            "$(".to_string(),
        ];

        // 默认允许的脚本目录（增加应用目录）
        let allowed_script_dirs = vec![
            "/opt/ops-scripts".to_string(),     // 默认运维脚本目录
            "/usr/local/bin/scripts".to_string(), // 本地脚本目录
            "/home/ops/scripts".to_string(),    // ops用户脚本目录
            "/tmp/ops-scripts".to_string(),     // 测试脚本目录
            "/tmp/apps".to_string(),            // 应用脚本目录
        ];

        // 允许的脚本文件扩展名
        let mut allowed_script_extensions = HashSet::new();
        allowed_script_extensions.insert("sh".to_string());
        allowed_script_extensions.insert("py".to_string());
        allowed_script_extensions.insert("pl".to_string());
        allowed_script_extensions.insert("rb".to_string());

        Self {
            allowed_commands,
            blocked_patterns,
            max_command_length: 1000,
            allowed_script_dirs,
            allowed_script_extensions,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationResult {
    Allowed,
    Blocked { reason: String },
}

impl CommandValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_predefined_commands() -> Vec<PredefinedCommand> {
        vec![
            // 系统信息类
            PredefinedCommand::new("ps aux", "查看所有进程", "系统信息", "显示所有运行中的进程详细信息"),
            PredefinedCommand::new("whoami", "查看当前用户", "系统信息", "显示当前登录的用户名"),
            PredefinedCommand::new("id", "查看用户ID信息", "系统信息", "显示当前用户的UID、GID等信息"),
            PredefinedCommand::new("hostname", "查看主机名", "系统信息", "显示系统主机名"),
            PredefinedCommand::new("uname -a", "查看系统信息", "系统信息", "显示系统内核和版本信息"),
            PredefinedCommand::new("date", "查看系统时间", "系统信息", "显示当前系统日期和时间"),
            PredefinedCommand::new("uptime", "查看系统运行时间", "系统信息", "显示系统运行时间和负载"),

            // 资源监控类
            PredefinedCommand::new("free -h", "查看内存使用", "资源监控", "以人类可读格式显示内存使用情况"),
            PredefinedCommand::new("df -h", "查看磁盘空间", "资源监控", "以人类可读格式显示磁盘使用情况"),
            PredefinedCommand::new("top -n 1", "查看进程资源使用", "资源监控", "显示当前进程CPU和内存使用情况"),
            PredefinedCommand::new("iostat", "查看IO统计", "资源监控", "显示磁盘IO统计信息"),
            PredefinedCommand::new("vmstat", "查看虚拟内存统计", "资源监控", "显示虚拟内存统计信息"),

            // 网络信息类
            PredefinedCommand::new("netstat -tlnp", "查看监听端口", "网络信息", "显示所有TCP监听端口"),
            PredefinedCommand::new("ss -tlnp", "查看socket连接", "网络信息", "显示socket连接信息"),
            PredefinedCommand::new("ip addr show", "查看网络接口", "网络信息", "显示网络接口配置"),
            PredefinedCommand::new("ping -c 4 8.8.8.8", "测试网络连通性", "网络信息", "测试到外网的网络连通性"),

            // 文件系统类
            PredefinedCommand::new("ls -la", "查看目录内容", "文件系统", "详细显示当前目录下的文件和文件夹"),
            PredefinedCommand::new("pwd", "查看当前路径", "文件系统", "显示当前工作目录的完整路径"),
            PredefinedCommand::new("find /var/log -name '*.log' -type f", "查找日志文件", "文件系统", "查找/var/log目录下的所有日志文件"),

            // 服务状态类
            PredefinedCommand::new("systemctl status", "查看服务状态", "服务管理", "显示系统服务的总体状态"),
            PredefinedCommand::new("journalctl -n 20", "查看系统日志", "服务管理", "显示最近20条系统日志"),

            // 环境信息类
            PredefinedCommand::new("env", "查看环境变量", "环境信息", "显示所有环境变量"),
            PredefinedCommand::new("which bash", "查找命令位置", "环境信息", "显示bash命令的完整路径"),

            // 脚本执行类
            PredefinedCommand::new("/tmp/ops-scripts/health-check.sh", "系统健康检查", "脚本执行", "执行系统健康检查脚本"),
            PredefinedCommand::new("/tmp/ops-scripts/disk-usage.py", "磁盘使用分析", "脚本执行", "分析磁盘使用情况"),
        ]
    }

    pub fn with_allowed_commands(mut self, commands: Vec<String>) -> Self {
        self.allowed_commands = commands.into_iter().collect();
        self
    }

    pub fn add_allowed_command(&mut self, command: String) {
        self.allowed_commands.insert(command);
    }

    pub fn add_blocked_pattern(&mut self, pattern: String) {
        self.blocked_patterns.push(pattern);
    }

    pub fn add_allowed_script_dir(&mut self, dir: String) {
        self.allowed_script_dirs.push(dir);
    }

    pub fn add_allowed_script_extension(&mut self, ext: String) {
        self.allowed_script_extensions.insert(ext);
    }

    pub fn with_allowed_script_dirs(mut self, dirs: Vec<String>) -> Self {
        self.allowed_script_dirs = dirs;
        self
    }

    pub fn validate(&self, command: &str) -> ValidationResult {
        // 检查命令长度
        if command.len() > self.max_command_length {
            return ValidationResult::Blocked {
                reason: format!("命令长度超过限制: {} > {}", command.len(), self.max_command_length),
            };
        }

        // 检查空命令
        if command.trim().is_empty() {
            return ValidationResult::Blocked {
                reason: "空命令".to_string(),
            };
        }

        // 检查是否是应用管理命令（特殊处理）
        if self.is_app_management_command(command) {
            return self.validate_app_management_command(command);
        }

        // 检查危险模式
        for pattern in &self.blocked_patterns {
            if command.to_lowercase().contains(&pattern.to_lowercase()) {
                return ValidationResult::Blocked {
                    reason: format!("包含危险模式: {}", pattern),
                };
            }
        }

        // 提取第一个命令词
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if let Some(base_command) = parts.first() {
            // 检查是否是脚本路径
            if self.is_script_path(base_command) {
                // 对脚本路径进行特殊验证
                match self.validate_script_path(base_command) {
                    ValidationResult::Allowed => {},
                    blocked => return blocked,
                }
            } else {
                // 检查是否在允许列表中
                if !self.allowed_commands.contains(*base_command) {
                    return ValidationResult::Blocked {
                        reason: format!("命令不在允许列表中: {}", base_command),
                    };
                }
            }
        } else {
            return ValidationResult::Blocked {
                reason: "无法解析命令".to_string(),
            };
        }

        ValidationResult::Allowed
    }

    pub fn sanitize_command(&self, command: &str) -> String {
        // 移除潜在的注入字符
        command
            .replace(";", " ")
            .replace("&&", " ")
            .replace("||", " ")
            .replace("|", " ")
            .replace("`", "")
            .replace("$", "")
            .replace("&", "")
            .trim()
            .to_string()
    }

    /// 检查是否为脚本路径（包含路径分隔符且不是纯命令名）
    fn is_script_path(&self, command: &str) -> bool {
        command.contains('/') || self.has_script_extension(command)
    }

    /// 检查文件是否有脚本扩展名
    fn has_script_extension(&self, path: &str) -> bool {
        if let Some(ext) = path.split('.').last() {
            self.allowed_script_extensions.contains(ext)
        } else {
            false
        }
    }

    /// 验证脚本路径是否安全
    fn validate_script_path(&self, script_path: &str) -> ValidationResult {
        use std::path::Path;
        
        // 检查路径是否为绝对路径
        let path = Path::new(script_path);
        if !path.is_absolute() {
            return ValidationResult::Blocked {
                reason: "只允许执行绝对路径的脚本".to_string(),
            };
        }

        // 检查路径规范化，防止路径遍历攻击
        let canonical_path = script_path.replace("../", "").replace("./", "");
        if canonical_path != script_path {
            return ValidationResult::Blocked {
                reason: "脚本路径包含危险的路径遍历字符".to_string(),
            };
        }

        // 检查是否在允许的目录中
        let mut allowed = false;
        for allowed_dir in &self.allowed_script_dirs {
            if script_path.starts_with(allowed_dir) {
                allowed = true;
                break;
            }
        }

        if !allowed {
            return ValidationResult::Blocked {
                reason: format!("脚本不在允许的目录中。允许的目录: {:?}", self.allowed_script_dirs),
            };
        }

        // 检查文件扩展名
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                if !self.allowed_script_extensions.contains(ext_str) {
                    return ValidationResult::Blocked {
                        reason: format!("不允许的脚本类型: .{}，允许的类型: {:?}", 
                                       ext_str, self.allowed_script_extensions),
                    };
                }
            }
        } else {
            return ValidationResult::Blocked {
                reason: "脚本文件必须有扩展名".to_string(),
            };
        }

        ValidationResult::Allowed
    }

    /// 获取允许的脚本目录列表
    pub fn get_allowed_script_dirs(&self) -> &Vec<String> {
        &self.allowed_script_dirs
    }

    /// 获取允许的脚本扩展名列表
    pub fn get_allowed_script_extensions(&self) -> Vec<String> {
        self.allowed_script_extensions.iter().cloned().collect()
    }

    /// 检查是否是应用管理命令
    fn is_app_management_command(&self, command: &str) -> bool {
        // 检查是否是cd /tmp/apps/xxx && bash xxx.sh命令模式
        (command.contains("cd /tmp/apps/") && command.contains("bash") && command.contains(".sh")) ||
        // 检查是否包含应用管理的特定模式
        (command.contains("/tmp/apps/") && (command.contains("kill") || command.contains("if") || command.contains("pid")))
    }

    /// 验证应用管理命令
    fn validate_app_management_command(&self, command: &str) -> ValidationResult {
        // 应用管理命令的特殊验证逻辑
        
        // 检查是否包含 /tmp/apps/ 路径
        if !command.contains("/tmp/apps/") {
            return ValidationResult::Blocked {
                reason: "应用管理命令必须在 /tmp/apps/ 目录下执行".to_string(),
            };
        }

        // 检查是否包含危险的命令注入
        if command.contains("rm -rf") || command.contains("format") || command.contains("dd") ||
           command.contains("curl") || command.contains("wget") || command.contains("nc ") ||
           command.contains("netcat") || command.contains("telnet") {
            return ValidationResult::Blocked {
                reason: "应用管理命令包含危险操作".to_string(),
            };
        }

        // 允许的应用管理操作模式
        let allowed_patterns = vec![
            r"cd /tmp/apps/[\w\-]+ && bash [\w\-]+\.sh (start|stop|status|update)",
            r"cd /tmp/apps/[\w\-]+ && if \[ -f [\w\-]+\.pid \]",
            r"kill \$\(cat [\w\-]+\.pid\)",
            r"rm -f [\w\-]+\.pid",
            r"ps -p \$pid",
        ];

        // 简化验证：检查关键词
        let has_valid_pattern = 
            (command.contains("bash") && command.contains(".sh")) ||
            (command.contains("kill") && command.contains("cat") && command.contains(".pid")) ||
            (command.contains("ps -p"));

        if has_valid_pattern {
            ValidationResult::Allowed
        } else {
            ValidationResult::Blocked {
                reason: "不支持的应用管理命令格式".to_string(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: std::time::SystemTime,
}

impl AuthToken {
    pub fn new(token: String, duration_secs: u64) -> Self {
        Self {
            token,
            expires_at: std::time::SystemTime::now() + std::time::Duration::from_secs(duration_secs),
        }
    }

    pub fn is_valid(&self) -> bool {
        std::time::SystemTime::now() < self.expires_at
    }

    pub fn matches(&self, token: &str) -> bool {
        self.is_valid() && self.token == token
    }
}

pub fn validate_auth_header(header_value: &str, expected_token: &str) -> bool {
    if let Some(token) = header_value.strip_prefix("Bearer ") {
        token == expected_token
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_validation_allowed() {
        let validator = CommandValidator::new();
        
        match validator.validate("ps aux") {
            ValidationResult::Allowed => {},
            ValidationResult::Blocked { reason } => panic!("Should be allowed: {}", reason),
        }
    }

    #[test]
    fn test_command_validation_blocked_dangerous() {
        let validator = CommandValidator::new();
        
        match validator.validate("rm -rf /") {
            ValidationResult::Blocked { reason } => {
                assert!(reason.contains("危险模式"));
            },
            ValidationResult::Allowed => panic!("Should be blocked"),
        }
    }

    #[test]
    fn test_command_validation_blocked_not_allowed() {
        let validator = CommandValidator::new();
        
        match validator.validate("malicious_command") {
            ValidationResult::Blocked { reason } => {
                assert!(reason.contains("不在允许列表中"));
            },
            ValidationResult::Allowed => panic!("Should be blocked"),
        }
    }

    #[test]
    fn test_command_sanitization() {
        let validator = CommandValidator::new();
        let sanitized = validator.sanitize_command("ps aux; rm -rf /");
        assert_eq!(sanitized, "ps aux  rm -rf /");
    }

    #[test]
    fn test_auth_token() {
        let token = AuthToken::new("test_token".to_string(), 3600);
        assert!(token.is_valid());
        assert!(token.matches("test_token"));
        assert!(!token.matches("wrong_token"));
    }

    #[test]
    fn test_auth_header_validation() {
        assert!(validate_auth_header("Bearer test_token", "test_token"));
        assert!(!validate_auth_header("Bearer wrong_token", "test_token"));
        assert!(!validate_auth_header("Invalid format", "test_token"));
    }
}