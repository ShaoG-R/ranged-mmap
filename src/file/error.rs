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
    
    /// Buffer too small for range
    /// 
    /// 缓冲区太小
    BufferTooSmall {
        buffer_len: usize,
        range_len: u64,
    },

}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::EmptyFile => write!(f, "Cannot map empty file / 无法映射空文件"),
            Error::BufferTooSmall { buffer_len, range_len } => {
                write!(
                    f,
                    "Buffer length {} is smaller than range length {} / 缓冲区长度 {} 小于范围长度 {}",
                    buffer_len, range_len, buffer_len, range_len
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
            Error::BufferTooSmall { .. } => io::Error::new(io::ErrorKind::InvalidInput, err.to_string())
        }
    }
}

/// Result type alias using our custom Error type
/// 
/// 使用自定义 Error 类型的 Result 类型别名
pub type Result<T> = std::result::Result<T, Error>;

