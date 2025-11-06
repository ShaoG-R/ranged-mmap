//! Unsafe lock-free file implementation based on memmap2
//! 
//! 基于 memmap2 的 Unsafe 无锁文件实现

use memmap2::MmapMut;
use std::cell::UnsafeCell;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;
use std::num::NonZeroU64;
use super::error::{Error, Result};

/// High-performance memory-mapped file (Unsafe lock-free version)
///
/// 基于内存映射的高性能文件（Unsafe 无锁版本）
///
/// ⚠️ **Warning**: This is an unsafe version. Users must ensure that concurrent writes
/// do not overlap.
/// 
/// ⚠️ **警告**：这是 unsafe 版本，用户需自行保证并发写入的范围不重叠。
/// 
/// It is recommended to use [`MmapFile`](super::MmapFile) + [`RangeAllocator`](super::RangeAllocator)
/// for compile-time safety.
/// 
/// 推荐使用 [`MmapFile`](super::MmapFile) + [`RangeAllocator`](super::RangeAllocator) 实现编译期安全。
///
/// This file implementation is optimized for concurrent random write scenarios.
///
/// 专为并发随机写入场景优化的文件实现。
///
/// # Features
///
/// - **Zero-copy writes**: Write operations directly modify mapped memory without system calls
/// - **Lock-free concurrency**: Concurrent writes to different regions require no locking for maximum performance
/// - **Reference counting**: Can be cloned and shared among multiple workers
/// - **Manual flushing**: Control when data is synchronized to disk for optimized batch operations
/// - **Runtime agnostic**: Does not depend on any specific async runtime
///
/// # 特性
///
/// - **零拷贝写入**：写入操作直接修改映射内存，无需系统调用
/// - **无锁并发**：不同区域的并发写入无需加锁，极致性能
/// - **引用计数**：可以克隆并在多个 worker 间共享
/// - **手动刷盘**：控制何时将数据同步到磁盘，优化批量操作
/// - **运行时无关**：不依赖特定异步运行时，可用于任何场景
///
/// # Limitations
///
/// - File size must be specified at creation and cannot be dynamically expanded
/// - Maximum file size is limited by system virtual memory
/// - ⚠️ Users must ensure that concurrent writes do not overlap (runtime responsibility)
///
/// # 限制
///
/// - 创建时必须指定文件大小，不支持动态扩展
/// - 文件大小上限受系统虚拟内存限制
/// - ⚠️ 用户需要确保不会并发写入重叠的内存区域（运行时责任）
///
/// # Safety Notes
///
/// This implementation uses `UnsafeCell` to allow lock-free concurrent writes. As long as:
/// - Different threads write to non-overlapping memory regions
/// - No reads occur to the same region during writes
/// 
/// It is completely safe. However, these guarantees must be maintained by the user.
///
/// # 安全性说明
///
/// 这个实现使用 `UnsafeCell` 来允许无锁并发写入。只要：
/// - 不同线程写入不重叠的内存区域
/// - 不在写入同时读取同一区域
/// 
/// 那么就是完全安全的。但这些保证需要用户自行维护。
///
/// # Examples
///
/// ```
/// # use ranged_mmap::{MmapFileInner, Result};
/// # use tempfile::tempdir;
/// # fn main() -> Result<()> {
/// # let dir = tempdir()?;
/// # let path = dir.path().join("download.bin");
/// # use std::num::NonZeroU64;
/// let file = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
///
/// // ⚠️ Users must ensure concurrent writes do not overlap
/// // ⚠️ 用户需自行保证不会并发写入重叠区域
/// let file1 = file.clone();
/// let file2 = file.clone();
///
/// std::thread::scope(|s| {
///     // Safety: Two threads write to non-overlapping regions [0, 512) and [512, 1024)
///     // Safety: 两个线程写入不重叠的区域 [0, 512) 和 [512, 1024)
///     s.spawn(|| unsafe { file1.write_at(0, &[1; 512]) });
///     s.spawn(|| unsafe { file2.write_at(512, &[2; 512]) });
/// });
///
/// unsafe { file.flush()?; }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MmapFileInner {
    /// Mutable reference to memory mapping, using UnsafeCell for interior mutability
    /// 
    /// 内存映射的可变引用，使用 UnsafeCell 允许内部可变性
    /// 
    /// # Safety
    /// Safe as long as different threads write to non-overlapping regions
    /// 
    /// # Safety
    /// 只要不同线程写入不重叠的区域，就是安全的
    mmap: Arc<UnsafeCell<MmapMut>>,
    
    /// File size in bytes
    /// 
    /// 文件大小
    size: NonZeroU64,
}

impl MmapFileInner {
    /// Create a new file and map it to memory
    ///
    /// 创建新文件并映射到内存
    ///
    /// If the file already exists, it will be truncated. The file will be pre-allocated
    /// to the specified size.
    ///
    /// 如果文件已存在会被截断。文件会被预分配到指定大小。
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
    /// # use ranged_mmap::{MmapFileInner, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// // Create a 10MB file
    /// // 创建 10MB 的文件
    /// let file = MmapFileInner::create(&path, NonZeroU64::new(10 * 1024 * 1024).unwrap())?;
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
    pub fn create(path: impl AsRef<Path>, size: NonZeroU64) -> Result<Self> {

        let path = path.as_ref();

        // Create file and pre-allocate size
        // 创建文件并预分配大小
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.set_len(size.get())?;

        // Create memory mapping
        // 创建内存映射
        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(Self {
            #[allow(clippy::arc_with_non_send_sync)]
            mmap: Arc::new(UnsafeCell::new(mmap)),
            size,
        })
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
    /// # use ranged_mmap::{MmapFileInner, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("existing.bin");
    /// # use std::num::NonZeroU64;
    /// # // Create file first
    /// # // 先创建文件
    /// # let _ = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
    /// let file = MmapFileInner::open(&path)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;

        let size = match file.metadata()?.len() {
            0 => return Err(Error::EmptyFile),
            size => NonZeroU64::new(size).unwrap(),
        };

        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(Self {
            #[allow(clippy::arc_with_non_send_sync)]
            mmap: Arc::new(UnsafeCell::new(mmap)),
            size,
        })
    }

    /// Write data at the specified position (lock-free operation)
    ///
    /// 在指定位置写入数据（无锁操作）
    ///
    /// This is an extremely fast operation that writes directly to mapped memory
    /// without requiring any locks.
    /// 
    /// 这是一个极快的操作，直接写入映射内存，不需要任何锁。
    /// 
    /// # Safety
    /// 
    /// The caller must ensure:
    /// - Different threads do not write to overlapping memory regions concurrently
    /// - No reads occur to the same region during writes
    ///
    /// Violating these constraints leads to data races, which is undefined behavior.
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保：
    /// - 不同线程不会并发写入重叠的内存区域
    /// - 不会在写入时读取同一区域
    ///
    /// 违反这些约束会导致数据竞争，这是未定义行为。
    ///
    /// # Parameters
    /// - `offset`: Write position (byte offset from file start)
    /// - `data`: Data to write
    ///
    /// # Returns
    /// Number of bytes actually written
    ///
    /// # 参数
    /// - `offset`: 写入位置（从文件开头的字节偏移）
    /// - `data`: 要写入的数据
    ///
    /// # 返回值
    /// 返回实际写入的字节数
    ///
    /// # Examples
    ///
    /// ```
    /// # use ranged_mmap::{MmapFileInner, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// let file = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
    ///
    /// // Concurrent writes to non-overlapping regions using std::thread
    /// // 使用 std::thread 并发写入不重叠区域
    /// let file1 = file.clone();
    /// let file2 = file.clone();
    ///
    /// std::thread::scope(|s| {
    ///     // Safety: Two threads write to non-overlapping regions [0, 5) and [100, 105)
    ///     // Safety: 两个线程写入不重叠的区域 [0, 5) 和 [100, 105)
    ///     s.spawn(|| unsafe { file1.write_at(0, b"hello") });
    ///     s.spawn(|| unsafe { file2.write_at(100, b"world") });
    /// });
    ///
    /// unsafe { file.flush()?; }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns `WriteExceedsFileSize` error if `offset + data.len()` exceeds file size
    ///
    /// # Errors
    /// 如果 `offset + data.len()` 超出文件大小，返回 `WriteExceedsFileSize` 错误
    #[inline]
    pub unsafe fn write_at(&self, offset: u64, data: &[u8]) -> Result<usize> {
        let offset_usize = offset as usize;
        let len = data.len();

        if offset_usize.saturating_add(len) > self.size.get() as usize {
            return Err(Error::WriteExceedsFileSize {
                offset,
                len,
                file_size: self.size.get(),
            });
        }

        // Safety: We assume the caller ensures different threads don't write to overlapping regions
        // Safety: 我们假设调用者确保不同线程不会写入重叠区域
        unsafe {
            let mmap = &mut *self.mmap.get();
            mmap[offset_usize..offset_usize + len].copy_from_slice(data);
        }

        Ok(len)
    }

    /// Write all data at the specified position
    ///
    /// 在指定位置写入所有数据
    ///
    /// This method guarantees that all data is written, or returns an error if
    /// insufficient space is available.
    ///
    /// 这个方法保证写入所有数据，如果空间不足会返回错误。
    ///
    /// # Safety
    /// 
    /// The caller must ensure:
    /// - Different threads do not write to overlapping memory regions concurrently
    /// - No reads occur to the same region during writes
    ///
    /// # Safety
    /// 
    /// 调用者需要确保：
    /// - 不同线程不会并发写入重叠的内存区域
    /// - 不会在写入时读取同一区域
    ///
    /// # Parameters
    /// - `offset`: Write position
    /// - `data`: Data to write
    ///
    /// # 参数
    /// - `offset`: 写入位置
    /// - `data`: 要写入的数据
    ///
    /// # Errors
    /// Returns `WriteExceedsFileSize` error if all data cannot be written (exceeds file size)
    ///
    /// # Errors
    /// 如果无法写入所有数据（超出文件大小），返回 `WriteExceedsFileSize` 错误
    #[inline]
    pub unsafe fn write_all_at(&self, offset: u64, data: &[u8]) -> Result<()> {
        unsafe { self.write_at(offset, data)?; }
        Ok(())
    }

    /// Read data at the specified position
    ///
    /// 在指定位置读取数据
    ///
    /// Reads data from the memory mapping into the buffer.
    ///
    /// 从内存映射中读取数据到缓冲区。
    ///
    /// # Safety
    /// 
    /// The caller must ensure no writes occur to the same region during reads.
    /// Concurrent reads are safe, but concurrent read-write to the same region
    /// leads to data races.
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保不会在读取时写入同一区域。
    /// 并发读取是安全的，但读写同一区域会导致数据竞争。
    ///
    /// # Parameters
    /// - `offset`: Read position
    /// - `buf`: Buffer to receive data
    ///
    /// # Returns
    /// Number of bytes actually read
    ///
    /// # 参数
    /// - `offset`: 读取位置
    /// - `buf`: 接收数据的缓冲区
    ///
    /// # 返回值
    /// 返回实际读取的字节数
    pub unsafe fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let offset_usize = offset as usize;
        let len = buf.len();

        if offset_usize >= self.size.get() as usize {
            return Ok(0);
        }

        let available = (self.size.get() as usize).saturating_sub(offset_usize).min(len);

        // Safety: Read operation is safe as long as no concurrent writes to the same region
        // Safety: 读取操作，只要不和写入同一区域并发就是安全的
        unsafe {
            let mmap = &*self.mmap.get();
            buf[..available].copy_from_slice(&mmap[offset_usize..offset_usize + available]);
        }

        Ok(available)
    }

    /// Flush data to disk asynchronously
    ///
    /// 异步刷新数据到磁盘
    ///
    /// Initiates an asynchronous flush operation without blocking for completion.
    /// The operating system will write data to disk in the background.
    ///
    /// 发起异步刷新操作，不会阻塞等待完成。操作系统会在后台将数据写入磁盘。
    ///
    /// # Safety
    /// 
    /// During the flush, the caller must ensure no other threads are modifying the
    /// mapped memory. While flush itself is a safe operation, it is marked unsafe
    /// for API consistency as it operates on data modified through unsafe methods.
    /// 
    /// # Safety
    /// 
    /// 在刷新期间，调用者需要确保没有其他线程正在修改映射的内存。
    /// 虽然 flush 本身是安全的操作，但为了保持 API 一致性，
    /// 它被标记为 unsafe，因为它操作的是通过 unsafe 方法修改的数据。
    ///
    /// # Examples
    ///
    /// ```
    /// # use ranged_mmap::{MmapFileInner, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// let file = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
    /// unsafe {
    ///     file.write_all_at(0, b"important data")?;
    ///     file.flush()?; // Flush asynchronously to disk
    ///                    // 异步刷新到磁盘
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub unsafe fn flush(&self) -> Result<()> {
        unsafe {
            let mmap = &*self.mmap.get();
            Ok(mmap.flush_async()?)
        }
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use ranged_mmap::{MmapFileInner, Result};
    /// # use tempfile::tempdir;
    /// # fn main() -> Result<()> {
    /// # let dir = tempdir()?;
    /// # let path = dir.path().join("output.bin");
    /// # use std::num::NonZeroU64;
    /// let file = MmapFileInner::create(&path, NonZeroU64::new(1024).unwrap())?;
    /// unsafe {
    ///     file.write_all_at(0, b"critical data")?;
    ///     file.sync_all()?; // Ensure data is written to disk
    ///                       // 确保数据已写入磁盘
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub unsafe fn sync_all(&self) -> Result<()> {
        unsafe {
            let mmap = &*self.mmap.get();
            Ok(mmap.flush()?)
        }
    }

    /// Flush a specific range to disk
    ///
    /// 刷新指定区域到磁盘
    ///
    /// Flushes only a portion of the file to disk, which can improve performance.
    ///
    /// 只刷新文件的一部分到磁盘，可以提高性能。
    ///
    /// # Safety
    /// 
    /// During the flush, the caller must ensure no other threads are modifying
    /// memory in that region.
    /// 
    /// # Safety
    /// 
    /// 在刷新期间，调用者需要确保没有其他线程正在修改该区域的内存。
    ///
    /// # Parameters
    /// - `offset`: Start position of the flush range
    /// - `len`: Length of the flush range
    ///
    /// # 参数
    /// - `offset`: 刷新区域的起始位置
    /// - `len`: 刷新区域的长度
    pub unsafe fn flush_range(&self, offset: u64, len: usize) -> Result<()> {
        let offset_usize = offset as usize;

        if offset_usize.saturating_add(len) > self.size.get() as usize {
            return Err(Error::FlushRangeExceedsFileSize {
                offset,
                len,
                file_size: self.size.get(),
            });
        }

        unsafe {
            let mmap = &*self.mmap.get();
            Ok(mmap.flush_async_range(offset_usize, len)?)
        }
    }

    /// Get file size
    /// 
    /// 获取文件大小
    #[inline]
    pub fn size(&self) -> NonZeroU64 {
        self.size
    }

    /// Fill the entire file with a specified byte
    ///
    /// 填充整个文件为指定字节
    ///
    /// Efficiently fills the entire file with the specified value.
    ///
    /// 高效地将整个文件填充为指定值。
    ///
    /// # Safety
    /// 
    /// The caller must ensure no other threads are reading or writing any part
    /// of the file during the fill. This operation modifies the entire file content.
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保在填充期间没有其他线程正在读写文件的任何部分。
    /// 此操作会修改整个文件内容。
    ///
    /// # Parameters
    /// - `byte`: Fill byte value
    ///
    /// # 参数
    /// - `byte`: 填充字节
    pub unsafe fn fill(&self, byte: u8) -> Result<()> {
        unsafe {
            let mmap = &mut *self.mmap.get();
            mmap.fill(byte);
        }
        Ok(())
    }

    /// Zero out the entire file
    ///
    /// 清零整个文件
    ///
    /// Fills the entire file with 0.
    ///
    /// 将整个文件填充为 0。
    ///
    /// # Safety
    /// 
    /// The caller must ensure no other threads are reading or writing any part
    /// of the file during the zeroing. This operation modifies the entire file content.
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保在清零期间没有其他线程正在读写文件的任何部分。
    /// 此操作会修改整个文件内容。
    pub unsafe fn zero(&self) -> Result<()> {
        unsafe { self.fill(0) }
    }

    /// Read a specific region into a new Vec
    ///
    /// 读取指定区域到新的 Vec
    ///
    /// This copies data into a new Vec.
    ///
    /// 这会拷贝数据到一个新的 Vec 中。
    ///
    /// # Safety
    /// 
    /// The caller must ensure no other threads are writing to the region during the read.
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保在读取期间没有其他线程正在写入该区域。
    ///
    /// # Parameters
    /// - `offset`: Read start position
    /// - `len`: Read length
    ///
    /// # 参数
    /// - `offset`: 读取起始位置
    /// - `len`: 读取长度
    pub unsafe fn read_slice(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let bytes_read = unsafe { self.read_at(offset, &mut buf)? };
        buf.truncate(bytes_read);
        Ok(buf)
    }

    /// Get a raw pointer to the underlying mmap
    /// 
    /// 获取底层 mmap 的原始指针
    /// 
    /// # Safety
    /// 
    /// The caller must ensure:
    /// - No multiple mutable references are created
    /// - The pointer lifetime does not exceed MmapFileInner
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保：
    /// - 不会创建多个可变引用
    /// - 指针的生命周期不会超过 MmapFileInner
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        unsafe {
            let mmap = &*self.mmap.get();
            mmap.as_ptr()
        }
    }

    /// Get a mutable raw pointer to the underlying mmap
    /// 
    /// 获取底层 mmap 的可变原始指针
    /// 
    /// # Safety
    /// 
    /// The caller must ensure:
    /// - No multiple mutable references are created
    /// - The pointer lifetime does not exceed MmapFileInner
    /// - No concurrent access to overlapping memory regions
    /// 
    /// # Safety
    /// 
    /// 调用者需要确保：
    /// - 不会创建多个可变引用
    /// - 指针的生命周期不会超过 MmapFileInner
    /// - 不会并发访问重叠的内存区域
    #[inline]
    pub unsafe fn as_mut_ptr(&self) -> *mut u8 {
        unsafe {
            let mmap = &mut *self.mmap.get();
            mmap.as_mut_ptr()
        }
    }
}

/// Implement Debug for MmapFileInner
/// 
/// 为 MmapFileInner 实现 Debug
impl std::fmt::Debug for MmapFileInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MmapFileInner")
            .field("size", &self.size)
            .field("mmap", &"MmapMut")
            .finish()
    }
}

// Implement Send and Sync
// Safety: Safe as long as users ensure different threads write to non-overlapping regions
// 
// 实现 Send 和 Sync
// Safety: 只要用户确保不同线程写入不重叠区域，就是安全的
unsafe impl Send for MmapFileInner {}
unsafe impl Sync for MmapFileInner {}

