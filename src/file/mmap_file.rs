//! Type-safe memory-mapped file implementation
//! 
//! 基于类型安全的内存映射文件实现

use super::allocator::RangeAllocator;
use super::mmap_file_inner::MmapFileInner;
use super::range::{AllocatedRange, WriteReceipt};
use super::error::{Error, Result};
use std::path::Path;
use std::num::NonZeroU64;

/// Type-safe memory-mapped file
/// 
/// 基于内存映射的安全文件
/// 
/// Achieves compile-time safety through [`RangeAllocator`] and [`AllocatedRange`].
/// 
/// 通过 [`RangeAllocator`] 和 [`AllocatedRange`] 实现编译期安全。
/// 
/// This version requires all write operations to provide an [`AllocatedRange`] parameter,
/// which can only be allocated through [`RangeAllocator`], thus guaranteeing at compile-time:
/// - All write ranges are valid (do not exceed file size)
/// - All ranges are non-overlapping (allocator allocates sequentially)
/// 
/// 这个版本要求所有写入操作提供 [`AllocatedRange`] 参数，
/// 该范围只能通过 [`RangeAllocator`] 分配，从而在编译期保证：
/// - 所有写入的范围都是有效的（不超出文件大小）
/// - 所有范围互不重叠（分配器顺序分配）
/// 
/// # Features
/// 
/// - **Compile-time safety**: Prevents overlapping writes through the type system
/// - **Zero-copy writes**: Write operations directly modify mapped memory
/// - **Lock-free concurrency**: Concurrent writes to different ranges require no locking
/// - **Reference counting**: Can be cloned and shared among multiple workers
/// - **Runtime agnostic**: Does not depend on any specific async runtime
/// 
/// # 特性
/// 
/// - **编译期安全**：通过类型系统防止重叠写入
/// - **零拷贝写入**：写入操作直接修改映射内存
/// - **无锁并发**：不同范围的并发写入无需加锁
/// - **引用计数**：可以克隆并在多个 worker 间共享
/// - **运行时无关**：不依赖特定异步运行时
/// 
/// # Usage Example
/// 
/// ```
/// # use ranged_mmap::{MmapFile, Result};
/// # use tempfile::tempdir;
/// # fn main() -> Result<()> {
/// # let dir = tempdir()?;
/// # let path = dir.path().join("output.bin");
/// # use std::num::NonZeroU64;
/// // Create file and allocator
/// // 创建文件和分配器
/// let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(1024).unwrap())?;
///
/// // Allocate ranges in the main thread
/// // 在主线程分配范围
/// let range1 = allocator.allocate(NonZeroU64::new(512).unwrap()).unwrap();
/// let range2 = allocator.allocate(NonZeroU64::new(512).unwrap()).unwrap();
///
/// // Concurrent writes to different ranges (compile-time safe!)
/// // 并发写入不同范围（编译期安全！）
/// std::thread::scope(|s| {
///     let f1 = file.clone();
///     let f2 = file.clone();
///     s.spawn(move || f1.write_range(range1, &[1; 512]));
///     s.spawn(move || f2.write_range(range2, &[2; 512]));
/// });
///
/// unsafe { file.sync_all()?; }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MmapFile {
    /// Underlying MmapFileInner implementation
    /// 
    /// 底层的 MmapFileInner 实现
    /// 
    /// # Safety
    /// AllocatedRange guarantees different threads write to non-overlapping regions
    /// 
    /// # Safety
    /// 通过 AllocatedRange 保证不同线程写入不重叠的区域
    inner: MmapFileInner,
}

impl MmapFile {
    /// Create a new file and return (MmapFile, RangeAllocator)
    /// 
    /// 创建新文件并返回 (MmapFile, RangeAllocator)
    /// 
    /// If the file already exists, it will be truncated. The file will be pre-allocated
    /// to the specified size.
    /// 
    /// 如果文件已存在会被截断。文件会被预分配到指定大小。
    /// 
    /// The returned tuple contains:
    /// - `MmapFile`: File handle that can be shared among multiple workers
    /// - `RangeAllocator`: Used to allocate ranges in the main thread
    /// 
    /// 返回的元组包含：
    /// - `MmapFile`: 可以被多个 worker 共享的文件句柄
    /// - `RangeAllocator`: 用于在主线程中分配范围
    /// 
    /// # Parameters
    /// - `path`: File path
    /// - `size`: File size in bytes, must be > 0
    /// 
    /// # 参数
    /// - `path`: 文件路径
    /// - `size`: 文件大小（字节），必须大于 0
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use ranged_mmap::{MmapFile, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(10 * 1024 * 1024).unwrap())?;
    ///
    /// // Allocate some ranges
    /// // 分配一些范围
    /// let range1 = allocator.allocate(NonZeroU64::new(1024).unwrap()).unwrap();
    /// let range2 = allocator.allocate(NonZeroU64::new(2048).unwrap()).unwrap();
    ///
    /// // Use file for concurrent writes
    /// // 使用 file 进行并发写入
    /// file.write_range(range1, &[0u8; 1024])?;
    /// file.write_range(range2, &[1u8; 2048])?;
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// # Errors
    /// - Returns `InvalidFileSize` error if size is 0
    /// - Returns corresponding I/O errors if file creation or memory mapping fails
    /// 
    /// # Errors
    /// - 如果 size 为 0，返回 `InvalidFileSize` 错误
    /// - 如果无法创建文件或映射内存，返回相应的 I/O 错误
    pub fn create(path: impl AsRef<Path>, size: NonZeroU64) -> Result<(Self, RangeAllocator)> {
        let inner = MmapFileInner::create(path, size)?;
        let allocator = RangeAllocator::new(size);
        Ok((Self { inner }, allocator))
    }

    /// Open an existing file and map it to memory
    /// 
    /// 打开已存在的文件并映射到内存
    /// 
    /// The file must already exist and have a size > 0.
    /// 
    /// 文件必须已存在且大小大于 0。
    /// 
    /// # Parameters
    /// - `path`: File path
    /// 
    /// # 参数
    /// - `path`: 文件路径
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use ranged_mmap::{MmapFile, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("existing.bin");
    /// # use std::num::NonZeroU64;
    /// # // Create file first
    /// # // 先创建文件
    /// # let _ = MmapFile::create(&path, NonZeroU64::new(1024).unwrap())?;
    /// let (file, mut allocator) = MmapFile::open(&path)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<(Self, RangeAllocator)> {
        let inner = MmapFileInner::open(path)?;
        let size = inner.size();
        let allocator = RangeAllocator::new(size);
        Ok((Self { inner }, allocator))
    }

    /// Write to an allocated range
    /// 
    /// 写入已分配的范围
    /// 
    /// The type system guarantees that ranges are valid and non-overlapping.
    /// Data length must exactly match the range length.
    /// 
    /// 通过类型系统保证范围是有效且不重叠的。
    /// 数据长度必须与范围长度完全匹配。
    /// 
    /// Returns a [`WriteReceipt`] upon successful write, which can be used for subsequent
    /// flush operations.
    /// 
    /// 成功写入后返回 [`WriteReceipt`] 凭据，可用于后续的刷新操作。
    /// 
    /// # Safety
    /// 
    /// This is a safe method because:
    /// - `AllocatedRange` can only be created through `RangeAllocator`
    /// - `RangeAllocator` allocates sequentially, guaranteeing non-overlapping ranges
    /// - Compile-time type checking prevents all potential data races
    /// 
    /// # Safety
    /// 
    /// 这是一个安全的方法，因为：
    /// - `AllocatedRange` 只能通过 `RangeAllocator` 创建
    /// - `RangeAllocator` 顺序分配，保证范围不重叠
    /// - 编译期类型检查防止了所有潜在的数据竞争
    /// 
    /// # Parameters
    /// - `range`: Allocated file range
    /// - `data`: Data to write, length must equal `range.len()`
    /// 
    /// # Returns
    /// Returns [`WriteReceipt`] proving the range has been successfully written
    /// 
    /// # 参数
    /// - `range`: 已分配的文件范围
    /// - `data`: 要写入的数据，长度必须等于 `range.len()`
    /// 
    /// # 返回值
    /// 返回 [`WriteReceipt`] 凭据，证明该范围已被成功写入
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use ranged_mmap::{MmapFile, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(1024).unwrap())?;
    ///
    /// // Allocate and write, obtaining a receipt
    /// // 分配并写入，获得凭据
    /// let range = allocator.allocate(NonZeroU64::new(100).unwrap()).unwrap();
    /// let receipt = file.write_range(range, &[42u8; 100])?;
    ///
    /// // Use receipt to flush
    /// // 使用凭据刷新
    /// file.flush_range(receipt)?;
    ///
    /// // Concurrent writes to different ranges
    /// // 并发写入不同范围
    /// let range1 = allocator.allocate(NonZeroU64::new(100).unwrap()).unwrap();
    /// let range2 = allocator.allocate(NonZeroU64::new(100).unwrap()).unwrap();
    ///
    /// let f1 = file.clone();
    /// let f2 = file.clone();
    ///
    /// std::thread::scope(|s| {
    ///     s.spawn(move || {
    ///         let receipt = f1.write_range(range1, &[1; 100]).unwrap();
    ///         f1.flush_range(receipt).unwrap();
    ///     });
    ///     s.spawn(move || {
    ///         let receipt = f2.write_range(range2, &[2; 100]).unwrap();
    ///         f2.flush_range(receipt).unwrap();
    ///     });
    /// });
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// # Errors
    /// Returns `DataLengthMismatch` error if data length does not match range length
    /// 
    /// # Errors
    /// 如果数据长度与范围长度不匹配，返回 `DataLengthMismatch` 错误
    #[inline]
    pub fn write_range(&self, range: AllocatedRange, data: &[u8]) -> Result<WriteReceipt> {
        // Check data length matches
        // 检查数据长度匹配
        if data.len() as u64 != range.len() {
            return Err(Error::DataLengthMismatch {
                data_len: data.len(),
                range_len: range.len(),
            });
        }

        // Safety: RangeAllocator guarantees non-overlapping ranges
        // Safety: RangeAllocator 保证范围不重叠
        unsafe { self.inner.write_at(range.start(), data)?; }

        // Return write receipt
        // 返回写入凭据
        Ok(WriteReceipt::new(range))
    }

    /// Write all data to the specified range
    /// 
    /// 在指定范围写入所有数据
    /// 
    /// This method is a convenience wrapper for `write_range` that guarantees
    /// all data is written or returns an error, and returns a write receipt.
    /// 
    /// 这个方法是 `write_range` 的便捷版本，
    /// 保证写入所有数据或返回错误，并返回写入凭据。
    /// 
    /// # Parameters
    /// - `range`: Allocated file range
    /// - `data`: Data to write
    /// 
    /// # Returns
    /// Returns [`WriteReceipt`] proving the range has been successfully written
    /// 
    /// # 参数
    /// - `range`: 已分配的文件范围
    /// - `data`: 要写入的数据
    /// 
    /// # 返回值
    /// 返回 [`WriteReceipt`] 凭据，证明该范围已被成功写入
    /// 
    /// # Errors
    /// Returns `DataLengthMismatch` error if data length does not match range length
    /// 
    /// # Errors
    /// 如果数据长度与范围长度不匹配，返回 `DataLengthMismatch` 错误
    #[inline]
    pub fn write_range_all(&self, range: AllocatedRange, data: &[u8]) -> Result<WriteReceipt> {
        self.write_range(range, data)
    }

    /// Get file size
    /// 
    /// 获取文件大小
    #[inline]
    pub fn size(&self) -> NonZeroU64 {
        self.inner.size()
    }

    /// Read data from the specified range
    /// 
    /// 在指定范围读取数据
    /// 
    /// Reads data from the memory mapping into the buffer.
    /// 
    /// 从内存映射中读取数据到缓冲区。
    /// 
    /// # Parameters
    /// - `range`: Range to read
    /// - `buf`: Buffer to receive data, length must be at least `range.len()`
    /// 
    /// # Returns
    /// Number of bytes actually read
    /// 
    /// # 参数
    /// - `range`: 要读取的范围
    /// - `buf`: 接收数据的缓冲区，长度必须至少为 `range.len()`
    /// 
    /// # 返回值
    /// 返回实际读取的字节数
    pub fn read_range(&self, range: AllocatedRange, buf: &mut [u8]) -> Result<usize> {
        let len = range.len() as usize;

        if buf.len() < len {
            return Err(Error::BufferTooSmall {
                buffer_len: buf.len(),
                range_len: range.len(),
            });
        }

        // Safety: Read operations are safe
        // Safety: 读取操作是安全的
        unsafe { self.inner.read_at(range.start(), &mut buf[..len]) }
    }

    /// Flush data to disk asynchronously
    /// 
    /// 异步刷新数据到磁盘
    /// 
    /// Initiates an asynchronous flush operation without blocking for completion.
    /// The operating system will write data to disk in the background.
    /// 
    /// 发起异步刷新操作，不会阻塞等待完成。操作系统会在后台将数据写入磁盘。
    pub fn flush(&self) -> Result<()> {
        unsafe { self.inner.flush() }
    }

    /// Flush data to disk synchronously
    /// 
    /// 同步刷新数据到磁盘
    /// 
    /// Synchronously flushes data in memory to disk, blocking until completion.
    /// This is slower than `flush()` but guarantees data has been written to disk.
    /// 
    /// 同步将内存中的数据刷新到磁盘，阻塞直到完成。
    /// 这比 `flush()` 慢，但保证数据已经写入磁盘。
    /// 
    /// # Safety
    /// 
    /// During the flush, the caller must ensure no other threads are modifying the
    /// mapped memory. While sync itself is a safe operation, it is marked unsafe
    /// for API consistency as it operates on data modified through unsafe methods.
    /// 
    /// # Safety
    /// 
    /// 在刷新期间，调用者需要确保没有其他线程正在修改映射的内存。
    /// 虽然 sync 本身是安全的操作，但为了保持 API 一致性，
    /// 它被标记为 unsafe，因为它操作的是通过 unsafe 方法修改的数据。
    pub unsafe fn sync_all(&self) -> Result<()> {
        unsafe { self.inner.sync_all() }
    }

    /// Flush a specific range to disk
    /// 
    /// 刷新指定区域到磁盘
    /// 
    /// Flushes only a portion of the file to disk, which can improve performance.
    /// 
    /// 只刷新文件的一部分到磁盘，可以提高性能。
    /// 
    /// By requiring a [`WriteReceipt`], this ensures only written ranges can be flushed,
    /// providing compile-time safety guarantees.
    /// 
    /// 通过要求 [`WriteReceipt`] 凭据，确保只能刷新已写入的范围，
    /// 提供编译期安全保证。
    /// 
    /// # Parameters
    /// - `receipt`: Write receipt proving the range has been successfully written
    /// 
    /// # 参数
    /// - `receipt`: 写入凭据，证明该范围已被成功写入
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use ranged_mmap::{MmapFile, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(1024).unwrap())?;
    /// let range = allocator.allocate(NonZeroU64::new(100).unwrap()).unwrap();
    ///
    /// // Write and get receipt
    /// // 写入并获得凭据
    /// let receipt = file.write_range(range, &[42u8; 100])?;
    ///
    /// // Can only flush ranges that have been written
    /// // 只能刷新已写入的范围
    /// file.flush_range(receipt)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn flush_range(&self, receipt: WriteReceipt) -> Result<()> {
        let range = receipt.range();
        unsafe { self.inner.flush_range(range.start(), range.len() as usize) }
    }
}

/// Implement Debug for MmapFile
/// 
/// 为 MmapFile 实现 Debug
impl std::fmt::Debug for MmapFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MmapFile")
            .field("inner", &self.inner)
            .finish()
    }
}

// Implement Send and Sync
// Safety: AllocatedRange guarantees different threads write to non-overlapping regions
// MmapFileInner already implements Send and Sync
// 
// 实现 Send 和 Sync
// Safety: AllocatedRange 保证不同线程写入不重叠区域
// MmapFileInner 已经实现了 Send 和 Sync
unsafe impl Send for MmapFile {}
unsafe impl Sync for MmapFile {}

