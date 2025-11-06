//! High-performance lock-free concurrent file writing library
//!
//! 高性能无锁并发文件写入库
//!
//! This library provides high-performance memory-mapped file implementations optimized for
//! concurrent random writes. It offers both type-safe and unsafe versions to balance safety
//! and performance needs.
//!
//! 本库提供针对并发随机写入优化的高性能内存映射文件实现。
//! 提供类型安全和 unsafe 版本以平衡安全性和性能需求。
//!
//! # Features
//!
//! - **Zero-copy writes**: Data is written directly to mapped memory
//! - **Lock-free concurrency**: No locks needed for writes to different regions
//! - **Type-safe API**: [`MmapFile`] prevents overlapping writes at compile-time
//! - **High performance**: Avoids frequent system calls
//! - **Runtime agnostic**: Works with any async runtime or without one
//!
//! # 特性
//!
//! - **零拷贝写入**：数据直接写入映射内存
//! - **无锁并发**：不同区域的写入无需加锁
//! - **类型安全 API**：[`MmapFile`] 在编译期防止重叠写入
//! - **高性能**：避免频繁的系统调用
//! - **运行时无关**：可用于任何异步运行时或无运行时环境
//!
//! # Quick Start
//!
//! ## Type-Safe Version (Recommended)
//!
//! Use [`MmapFile`] with [`RangeAllocator`] for compile-time safety:
//!
//! ## 类型安全版本（推荐）
//!
//! 使用 [`MmapFile`] 和 [`RangeAllocator`] 获得编译期安全：
//!
//! ```
//! use ranged_mmap::{MmapFile, RangeAllocator, Result};
//! # use tempfile::tempdir;
//! # fn main() -> Result<()> {
//! # let dir = tempdir()?;
//! # let path = dir.path().join("output.bin");
//! # use std::num::NonZeroU64;
//!
//! // Create file and allocator
//! // 创建文件和分配器
//! let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(1024).unwrap())?;
//!
//! // Allocate ranges
//! // 分配范围
//! let range1 = allocator.allocate(NonZeroU64::new(512).unwrap()).unwrap();
//! let range2 = allocator.allocate(NonZeroU64::new(512).unwrap()).unwrap();
//!
//! // Concurrent writes (compile-time safe!)
//! // 并发写入（编译期安全！）
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
//! ## Unsafe Version (Maximum Performance)
//!
//! Use [`MmapFileInner`] when you can guarantee safety:
//!
//! ## Unsafe 版本（最大性能）
//!
//! 当你能保证安全时使用 [`MmapFileInner`]：
//!
//! ```
//! use ranged_mmap::{MmapFileInner, Result};
//! # use tempfile::tempdir;
//! # fn main() -> Result<()> {
//! # let dir = tempdir()?;
//! # let path = dir.path().join("download.bin");
//! # use std::num::NonZeroU64;
//!
//! let file = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
//!
//! // ⚠️ You must ensure non-overlapping writes
//! // ⚠️ 你必须确保写入不重叠
//! let file1 = file.clone();
//! let file2 = file.clone();
//!
//! std::thread::scope(|s| {
//!     // Safety: Non-overlapping regions
//!     // Safety: 不重叠的区域
//!     s.spawn(|| unsafe { file1.write_at(0, &[1; 512]) });
//!     s.spawn(|| unsafe { file2.write_at(512, &[2; 512]) });
//! });
//!
//! unsafe { file.flush()?; }
//! # Ok(())
//! # }
//! ```
//!
//! # Main Types
//!
//! - [`MmapFile`]: Type-safe memory-mapped file
//! - [`MmapFileInner`]: Unsafe high-performance memory-mapped file
//! - [`RangeAllocator`]: Allocates non-overlapping file ranges
//! - [`AllocatedRange`]: Represents an allocated file range
//! - [`WriteReceipt`]: Proof that a range has been written
//!
//! # 主要类型
//!
//! - [`MmapFile`][]: 类型安全的内存映射文件
//! - [`MmapFileInner`]: Unsafe 高性能内存映射文件
//! - [`RangeAllocator`][]: 分配不重叠的文件范围
//! - [`AllocatedRange`][]: 表示已分配的文件范围
//! - [`WriteReceipt`][]: 证明范围已被写入的凭据

mod file;

pub use file::{AllocatedRange, Error, MmapFile, MmapFileInner, RangeAllocator, Result, WriteReceipt};