use std::fs;
use ops_common::VersionInfo;

pub fn read_app_versions(base_dir: &str) -> Vec<VersionInfo> {
    let mut version_infos = Vec::new();

    match fs::read_dir(base_dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    if path.is_dir() {
                        let version_path = path.join("version.txt");
                        if version_path.exists() {
                            match fs::read_to_string(&version_path) {
                                Ok(contents) => {
                                    match serde_json::from_str::<VersionInfo>(&contents) {
                                        Ok(info) => version_infos.push(info),
                                        Err(e) => tracing::debug!("无法解析 {}: {}", version_path.display(), e),
                                    }
                                }
                                Err(e) => tracing::debug!("无法读取 {}: {}", version_path.display(), e),
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            // 不输出错误，因为目录可能不存在是正常的
            tracing::debug!("无法读取目录 {}: {}", base_dir, e);
        }
    }

    version_infos
}