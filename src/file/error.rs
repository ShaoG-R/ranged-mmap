//! Error types for ranged-mmap
//! 
//! ranged-mmap 的错误类型

use std::fmt;
use std::io;

/// Error type for ranged-mmap operations
/// 
/// ranged-mmap 操作的错误类型
#[derive(Debug)]
pub enum Error {
    /// I/O error
    /// 
    /// I/O 错误
    Io(io::Error),
    
    /// Empty file cannot be mapped
    /// 
    /// 空文件无法映射
    EmptyFile,
    
    /// Invalid range (start > end or empty range)
    /// 
    /// 无效的范围（start > end 或空范围）
    InvalidRange { start: u64, end: u64 },
    
    /// Write would exceed file size
    /// 
    /// 写入会超出文件大小
    WriteExceedsFileSize {
        offset: u64,
        len: usize,
        file_size: u64,
    },
    
    /// Data length doesn't match range length
    /// 
    /// 数据长度与范围长度不匹配
    DataLengthMismatch {
        data_len: usize,
        range_len: u64,
    },
    
    /// Buffer too small for range
    /// 
    /// 缓冲区太小
    BufferTooSmall {
        buffer_len: usize,
        range_len: u64,
    },
    
    /// Flush range exceeds file size
    /// 
    /// 刷新范围超出文件大小
    FlushRangeExceedsFileSize {
        offset: u64,
        len: usize,
        file_size: u64,
    },
    
    /// Insufficient space for allocation
    /// 
    /// 分配空间不足
    InsufficientSpace {
        requested: u64,
        available: u64,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::EmptyFile => write!(f, "Cannot map empty file / 无法映射空文件"),
            Error::InvalidRange { start, end } => {
                write!(f, "Invalid range: start={}, end={} (start must be <= end) / 无效的范围：start={}, end={}（start 必须小于等于 end）", start, end, start, end)
            }
            Error::WriteExceedsFileSize { offset, len, file_size } => {
                write!(
                    f,
                    "Write would exceed file size: offset={}, len={}, file_size={} / 写入会超出文件大小：offset={}, len={}, file_size={}",
                    offset, len, file_size, offset, len, file_size
                )
            }
            Error::DataLengthMismatch { data_len, range_len } => {
                write!(
                    f,
                    "Data length {} doesn't match range length {} / 数据长度 {} 与范围长度 {} 不匹配",
                    data_len, range_len, data_len, range_len
                )
            }
            Error::BufferTooSmall { buffer_len, range_len } => {
                write!(
                    f,
                    "Buffer length {} is smaller than range length {} / 缓冲区长度 {} 小于范围长度 {}",
                    buffer_len, range_len, buffer_len, range_len
                )
            }
            Error::FlushRangeExceedsFileSize { offset, len, file_size } => {
                write!(
                    f,
                    "Flush range exceeds file size: offset={}, len={}, file_size={} / 刷新范围超出文件大小：offset={}, len={}, file_size={}",
                    offset, len, file_size, offset, len, file_size
                )
            }
            Error::InsufficientSpace { requested, available } => {
                write!(
                    f,
                    "Insufficient space: requested {} bytes, available {} bytes / 空间不足：请求 {} 字节，可用 {} 字节",
                    requested, available, requested, available
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

/// Convert from io::Error to Error
/// 
/// 从 io::Error 转换到 Error
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

/// Convert from Error to io::Error for compatibility
/// 
/// 从 Error 转换到 io::Error 以保持兼容性
impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Io(io_err) => io_err,
            Error::EmptyFile => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
            Error::InvalidRange { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
            Error::WriteExceedsFileSize { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
            Error::DataLengthMismatch { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
            Error::BufferTooSmall { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
            Error::FlushRangeExceedsFileSize { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
            Error::InsufficientSpace { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
        }
    }
}

/// Result type alias using our custom Error type
/// 
/// 使用自定义 Error 类型的 Result 类型别名
pub type Result<T> = std::result::Result<T, Error>;

