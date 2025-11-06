//! High-performance file implementation based on memmap2
//!
//! 基于 memmap2 的高性能文件实现
//!
//! Provides two implementations:
//! - [`MmapFileInner`]: High-performance unsafe version, users must ensure concurrent safety
//! - [`MmapFile`]: Compile-time safe version, guarantees no overlapping writes through type system
//!
//! 提供两种实现：
//! - [`MmapFileInner`]: 高性能的 unsafe 版本，用户需自行保证并发安全
//! - [`MmapFile`]: 编译期安全版本，通过类型系统保证不会重叠写入
//!
//! # Performance Benefits
//!
//! Uses memory mapping technology to optimize random write scenarios. Advantages over
//! traditional file I/O:
//! 1. Zero-copy: Data is written directly to mapped memory, OS manages flushing
//! 2. High performance: Avoids frequent system calls
//! 3. Simplified code: No need for explicit seek operations
//! 4. Lock-free concurrency: Writes to different regions require no locking
//! 5. Runtime agnostic: Does not depend on any specific async runtime
//!
//! # 性能优势
//!
//! 使用内存映射技术优化随机写入场景，相比传统文件 I/O 的优势：
//! 1. 零拷贝：数据直接写入映射内存，由操作系统管理刷盘
//! 2. 高性能：避免频繁的系统调用
//! 3. 简化代码：不需要显式的 seek 操作
//! 4. 无锁并发：不同区域的写入不需要加锁
//! 5. 运行时无关：不依赖特定的异步运行时
//!
//! # Recommended Usage
//!
//! For new code, it is recommended to use the [`MmapFile`] + [`RangeAllocator`] combination,
//! which ensures concurrent safety through compile-time type checking:
//!
//! # 推荐用法
//!
//! 对于新代码，推荐使用 [`MmapFile`] + [`RangeAllocator`] 的组合，
//! 通过编译期类型检查保证并发安全：
//!
//! ```
//! # use ranged_mmap::{MmapFile, Result};
//! # use tempfile::tempdir;
//! # fn main() -> Result<()> {
//! # let dir = tempdir()?;
//! # let path = dir.path().join("output.bin");
//! # use std::num::NonZeroU64;
//! // Create file and allocator
//! // 创建文件和分配器
//! let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(1024).unwrap())?;
//!
//! // Allocate ranges in the main thread
//! // 在主线程分配范围
//! let range1 = allocator.allocate(NonZeroU64::new(512).unwrap()).unwrap();
//! let range2 = allocator.allocate(NonZeroU64::new(512).unwrap()).unwrap();
//!
//! // Concurrent writes to different ranges (compile-time safe!)
//! // 并发写入不同范围（编译期安全！）
//! std::thread::scope(|s| {
//!     let f1 = file.clone();
//!     let f2 = file.clone();
//!     s.spawn(move || f1.write_range(range1, &[1; 512]));
//!     s.spawn(move || f2.write_range(range2, &[2; 512]));
//! });
//!
//! unsafe { file.sync_all()?; }
//! # Ok(())
//! # }
//! ```
//!
//! # Unsafe Version
//!
//! If you need maximum performance and can guarantee concurrent safety yourself,
//! you can use [`MmapFileInner`]:
//!
//! # Unsafe 版本
//!
//! 如果你需要最大性能并且能够保证并发安全，可以使用 [`MmapFileInner`]：
//!
//! ```
//! # use ranged_mmap::{MmapFileInner, Result};
//! # use tempfile::tempdir;
//! # fn main() -> Result<()> {
//! # let dir = tempdir()?;
//! # let path = dir.path().join("download.bin");
//! # use std::num::NonZeroU64;
//! let file = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
//!
//! // ⚠️ Users must ensure concurrent writes do not overlap
//! // ⚠️ 用户需自行保证不会并发写入重叠区域
//! let file1 = file.clone();
//! let file2 = file.clone();
//!
//! std::thread::scope(|s| {
//!     // Safety: Two threads write to non-overlapping regions [0, 512) and [512, 1024)
//!     // Safety: 两个线程写入不重叠的区域 [0, 512) 和 [512, 1024)
//!     s.spawn(|| unsafe { file1.write_at(0, &[1; 512]) });
//!     s.spawn(|| unsafe { file2.write_at(512, &[2; 512]) });
//! });
//!
//! unsafe { file.flush()?; }
//! # Ok(())
//! # }
//! ```

mod allocator;
mod error;
mod mmap_file;
mod mmap_file_inner;
mod range;

#[cfg(test)]
mod tests;

// Re-export public API
// 重新导出公共 API
pub use allocator::RangeAllocator;
pub use error::{Error, Result};
pub use mmap_file::MmapFile;
pub use mmap_file_inner::MmapFileInner;
pub use range::{AllocatedRange, WriteReceipt};
