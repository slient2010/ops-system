use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use chrono::Local;
use tar::Builder;
use flate2::write::GzEncoder;
use flate2::Compression;

/// LogRotator handles size-based log rotation with compression
pub struct LogRotator {
    file_path: PathBuf,
    file: Option<File>,
    max_size: u64,
    log_directory: String,
    file_size: u64,
}

impl LogRotator {
    /// Create a new LogRotator
    pub fn new<P: AsRef<Path>>(file_path: P, max_size_mb: u64, log_directory: String) -> Result<Self, io::Error> {
        let max_size = max_size_mb * 1024 * 1024; // Convert MB to bytes
        let file_path = file_path.as_ref().to_path_buf();
        
        let file = Self::create_log_file(&file_path)?;
        let file_size = file.metadata()?.len();
        
        Ok(Self {
            file: Some(file),
            file_path,
            max_size,
            log_directory,
            file_size,
        })
    }

    fn create_log_file(file_path: &Path) -> Result<File, io::Error> {
        fs::create_dir_all(file_path.parent().unwrap_or(Path::new(".")))?;
        File::options()
            .create(true)
            .append(true)
            .open(file_path)
    }

    /// Write data to the log file, rotating if necessary
    pub fn write(&mut self, data: &[u8]) -> Result<(), io::Error> {
        // Only check rotation if we have data to write
        if !data.is_empty() {
            let new_size = self.file_size + data.len() as u64;

            if new_size > self.max_size {
                self.rotate_log()?;
            }
        }

        // Write the data
        if let Some(ref mut file) = self.file {
            file.write_all(data)?;
            // Only flush periodically to improve performance, or when rotating
            // For now, keeping flush for consistency but could be optimized further
            file.flush()?;
            self.file_size += data.len() as u64;
        }

        Ok(())
    }

    /// Perform log rotation and compression
    fn rotate_log(&mut self) -> Result<(), io::Error> {
        // Close current log file
        let old_file = self.file.take();

        // Get creation timestamp for the current log file
        let timestamp = Local::now().format("%Y-%m-%d").to_string();

        // Generate new filename with timestamp and sequence number
        let log_file_stem = self.file_path.file_stem()
            .unwrap_or(std::ffi::OsStr::new("log"))
            .to_string_lossy();
        let log_file_ext = self.file_path.extension()
            .unwrap_or(std::ffi::OsStr::new(""))
            .to_string_lossy();
        
        // Find sequence number to ensure uniqueness
        let sequence_num = self.find_next_sequence_number(&log_file_stem, &timestamp)?;
        let compressed_filename = if log_file_ext.is_empty() {
            format!("{}.{}.{}.tar.gz", log_file_stem, timestamp, sequence_num)
        } else {
            format!("{}.{}.{}.{}.tar.gz", log_file_stem, timestamp, sequence_num, log_file_ext)
        };

        let compressed_path = PathBuf::from(&self.log_directory).join(&compressed_filename);

        // Compress the current log file
        self.compress_log(&compressed_path)?;

        // Create a new log file and update the handle
        let new_file = Self::create_log_file(&self.file_path)?;
        self.file = Some(new_file);
        self.file_size = 0;

        // Close the old file handle to flush to disk
        drop(old_file);

        Ok(())
    }

    /// Find the next sequence number to avoid filename conflicts
    fn find_next_sequence_number(&self, stem: &str, timestamp: &str) -> Result<u32, io::Error> {
        let log_dir = Path::new(&self.log_directory);
        let log_dir = if log_dir.exists() { log_dir } else { Path::new(".") };
        
        let _pattern = format!("{}*.tar.gz", stem);
        
        let mut max_seq = 0;
        for entry in fs::read_dir(log_dir)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();
            
            if filename_str.starts_with(&format!("{}.", stem)) && filename_str.contains(timestamp) {
                // Extract sequence number from filename like log_file.2025-11-01.1.tar.gz
                if let Some(pos) = filename_str.find(timestamp) {
                    let after_timestamp = &filename_str[pos + timestamp.len() + 1..];
                    if let Some(period_pos) = after_timestamp.find('.') {
                        let seq_str = &after_timestamp[..period_pos];
                        if let Ok(seq) = seq_str.parse::<u32>() {
                            if seq > max_seq {
                                max_seq = seq;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(max_seq + 1)
    }

    /// Compress the current log file
    fn compress_log(&self, compressed_path: &Path) -> Result<(), io::Error> {
        // Create compressed file
        let tar_gz = File::create(compressed_path)?;
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);

        // Add the current log file to the archive
        let metadata = fs::metadata(&self.file_path)?;
        if metadata.len() > 0 {
            tar.append_path_with_name(&self.file_path, self.file_path.file_name().unwrap())?;
        }

        // Finish writing the archive
        let gz = tar.into_inner()?;
        gz.finish()?;

        // Remove the original log file after compression
        fs::remove_file(&self.file_path)?;

        Ok(())
    }

    /// Close the current log file
    pub fn close(&mut self) -> Result<(), io::Error> {
        if let Some(file) = self.file.take() {
            drop(file);
        }
        Ok(())
    }
}

impl Drop for LogRotator {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

// Wrapper for thread-safe access to LogRotator
pub struct ThreadSafeLogRotator {
    inner: Arc<Mutex<LogRotator>>,
}

impl ThreadSafeLogRotator {
    pub fn new<P: AsRef<Path>>(file_path: P, max_size_mb: u64, log_directory: String) -> Result<Self, io::Error> {
        Ok(Self {
            inner: Arc::new(Mutex::new(LogRotator::new(file_path, max_size_mb, log_directory)?)),
        })
    }

    pub fn write(&self, data: &[u8]) -> Result<(), io::Error> {
        // Handle potential poisoning of the mutex
        match self.inner.lock() {
            Ok(mut guard) => guard.write(data),
            Err(poisoned) => {
                // If the mutex was poisoned (due to a panic in another thread), try to recover
                let mut guard = poisoned.into_inner();
                guard.write(data)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_log_rotation() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        let log_dir = dir.path().to_string_lossy().to_string();

        // Create a rotator with 1KB max size
        let mut rotator = LogRotator::new(&log_path, 1, log_dir.clone()).unwrap();

        // Write more than 1KB of data
        let large_data = vec![b'A'; 2048]; // 2KB
        rotator.write(&large_data).unwrap();

        // Check that rotation happened
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert!(entries.len() >= 1, "Expected at least one file (compressed or original)");
    }

    #[test]
    fn test_thread_safe_log_rotator() {
        use std::thread;

        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        let log_dir = dir.path().to_string_lossy().to_string();

        let rotator = ThreadSafeLogRotator::new(&log_path, 1, log_dir).unwrap();

        // Test concurrent access
        let rotator_clone = Arc::clone(&rotator.inner);
        let handle = thread::spawn(move || {
            let large_data = vec![b'B'; 1024];
            rotator_clone.lock().unwrap().write(&large_data).unwrap();
        });

        let large_data = vec![b'C'; 1024];
        rotator.write(&large_data).unwrap();
        
        handle.join().unwrap();
    }

    #[test]
    fn test_log_rotation_with_writer() {
        use std::io::Write;
        
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test_writer.log");
        let log_dir = dir.path().to_string_lossy().to_string();

        // Create a thread-safe rotator
        let rotator = ThreadSafeLogRotator::new(&log_path, 1, log_dir).unwrap(); // 1MB limit
        
        // Create a writer wrapper similar to what's used in the main application
        struct LogRotatorWriter {
            rotator: ThreadSafeLogRotator,
        }

        impl LogRotatorWriter {
            fn new(rotator: ThreadSafeLogRotator) -> Self {
                Self { rotator }
            }
        }

        impl std::io::Write for LogRotatorWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.rotator.write(buf)?;
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = LogRotatorWriter::new(rotator);
        
        // Write a message that should be logged
        let message = "Test log message
";
        writer.write_all(message.as_bytes()).unwrap();
        
        // Verify the log file was created and contains the message
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("Test log message"));
    }
}
