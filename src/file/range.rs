//! File range and write receipt types
//! 
//! 文件范围和写入凭据类型

use std::{num::NonZeroU64, ops::Range};
use super::error::{Error, Result};

/// Allocated file range
/// 
/// 已分配的文件范围
/// 
/// Represents a valid range `[start, end)` allocated through [`RangeAllocator`](super::RangeAllocator).
/// This type can only be created through the allocator, guaranteeing that all ranges are non-overlapping.
/// 
/// 表示通过 [`RangeAllocator`](super::RangeAllocator) 分配的有效范围 `[start, end)`。
/// 此类型只能通过分配器创建，保证所有范围不重叠。
/// 
/// # Range Format
/// 
/// Uses half-open interval `[start, end)`:
/// - `start`: Inclusive start position
/// - `end`: Exclusive end position
/// 
/// For example: `AllocatedRange { start: 0, end: 10 }` represents bytes 0-9 (10 bytes total)
/// 
/// # 范围格式
/// 
/// 使用左闭右开区间 `[start, end)`：
/// - `start`: 包含的起始位置
/// - `end`: 不包含的结束位置
/// 
/// 例如：`AllocatedRange { start: 0, end: 10 }` 表示字节 0-9（共 10 字节）
/// 
/// # Safety Guarantees
/// 
/// - `start` is always ≤ `end`
/// - Can only be created through the allocator, preventing overlaps
/// - Provides immutable access, preventing modification
/// 
/// # 安全性保证
/// 
/// - `start` 总是小于等于 `end`
/// - 只能通过分配器创建，防止重叠
/// - 提供不可变访问，防止修改
/// 
/// # Examples
/// 
/// ```
/// # use ranged_mmap::{MmapFile, RangeAllocator, Result};
/// # use tempfile::tempdir;
/// # fn main() -> Result<()> {
/// # let dir = tempdir()?;
/// # let path = dir.path().join("output.bin");
/// # use std::num::NonZeroU64;
/// let (file, mut allocator) = MmapFile::create(&path, NonZeroU64::new(100).unwrap())?;
/// let range = allocator.allocate(NonZeroU64::new(10).unwrap()).unwrap();
///
/// // Get range information
/// // 获取范围信息
/// assert_eq!(range.start(), 0);
/// assert_eq!(range.end(), 10);
/// assert_eq!(range.len(), 10);
///
/// let (start, end) = range.as_range_tuple();
/// assert_eq!(start, 0);
/// assert_eq!(end, 10);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatedRange {
    /// Range start position (inclusive)
    /// 
    /// 范围起始位置（包含）
    start: u64,
    
    /// Range end position (exclusive)
    /// 
    /// 范围结束位置（不包含）
    end: u64,
}

impl AllocatedRange {
    /// Internal constructor (crate-visible only, no validation)
    /// 
    /// 内部构造函数（仅 crate 内可见，不进行验证）
    /// 
    /// Creates a range using half-open interval `[start, end)`. No validation is performed.
    /// 
    /// 使用左闭右开区间 `[start, end)` 创建范围。不进行验证。
    #[inline]
    pub(crate) fn from_range_unchecked(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    /// Get the start position
    /// 
    /// 获取起始位置
    /// 
    /// # Returns
    /// The start position of the range (inclusive)
    /// 
    /// # 返回值
    /// 返回范围的起始位置（包含）
    #[inline]
    pub fn start(&self) -> u64 {
        self.start
    }

    /// Get the end position
    /// 
    /// 获取结束位置
    /// 
    /// # Returns
    /// The end position of the range (exclusive)
    /// 
    /// # 返回值
    /// 返回范围的结束位置（不包含）
    #[inline]
    pub fn end(&self) -> u64 {
        self.end
    }

    /// Get the length of the range in bytes
    /// 
    /// 获取范围的长度（字节数）
    #[inline]
    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    /// Check if the range is empty
    /// 
    /// 检查范围是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }


    /// Split the range at the given position
    /// 
    /// 在给定位置拆分范围
    /// 
    /// # Parameters
    /// - `pos`: Position to split at
    /// 
    /// # Returns
    /// Two new ranges, or an error if the split would create invalid ranges
    /// 
    /// # 参数
    /// - `pos`: 拆分位置
    /// 
    /// # 返回值
    /// 两个新的范围，如果拆分会创建无效范围则返回错误
    #[inline]
    pub fn split_at(&self, pos: NonZeroU64) -> Result<(AllocatedRange, AllocatedRange)> {
        let start = self.start;
        let end = self.end;

        if pos.get() >= end {
            return Err(Error::InvalidRange { start, end });
        }
        Ok((AllocatedRange::from_range_unchecked(start, pos.get()), AllocatedRange::from_range_unchecked(pos.get(), end)))
    }

    /// Get the range as a tuple (start, end)
    /// 
    /// 获取范围的元组表示 (start, end)
    /// 
    /// Returns half-open interval `(start, end)`.
    /// 
    /// 返回左闭右开区间 `(start, end)`。
    #[inline]
    pub fn as_range_tuple(&self) -> (u64, u64) {
        (self.start, self.end)
    }

    /// Convert to standard Range<u64>
    /// 
    /// 转换为标准 Range<u64>
    /// 
    /// Returns half-open interval `start..end`.
    /// 
    /// 返回左闭右开区间 `start..end`。
    #[inline]
    pub fn as_range(&self) -> Range<u64> {
        self.start..self.end
    }
}

impl From<AllocatedRange> for Range<u64> {
    #[inline]
    fn from(range: AllocatedRange) -> Self {
        range.as_range()
    }
}

/// Write receipt
/// 
/// 写入凭据
/// 
/// A receipt proving that a range has been successfully written.
/// 
/// This receipt can only be obtained after successfully writing through
/// [`MmapFile::write_range`](super::MmapFile::write_range), and can only be used to flush
/// the corresponding range. This provides compile-time safety guarantees:
/// - Can only flush ranges that have been written
/// - Cannot flush ranges that have not been written
/// 
/// 证明某个范围已被成功写入的凭据。
/// 
/// 只有通过 [`MmapFile::write_range`](super::MmapFile::write_range) 成功写入后才能获得此凭据，
/// 并且只能用于刷新对应的范围。这提供了编译期的安全保证：
/// - 只能刷新已写入的范围
/// - 不能刷新未写入的范围
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
/// // Use receipt to flush the range
/// // 使用凭据刷新该范围
/// file.flush_range(receipt)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteReceipt {
    /// The range that was written
    /// 
    /// 已写入的范围
    range: AllocatedRange,
}

impl WriteReceipt {
    /// Internal constructor (crate-visible only)
    /// 
    /// 内部构造函数（仅 crate 内可见）
    #[inline]
    pub(crate) fn new(range: AllocatedRange) -> Self {
        Self { range }
    }

    /// Get the range corresponding to this receipt
    /// 
    /// 获取凭据对应的范围
    #[inline]
    pub fn range(&self) -> AllocatedRange {
        self.range
    }

    /// Get the start position of the range
    /// 
    /// 获取范围的起始位置
    #[inline]
    pub fn start(&self) -> u64 {
        self.range.start()
    }

    /// Get the end position of the range
    /// 
    /// 获取范围的结束位置
    #[inline]
    pub fn end(&self) -> u64 {
        self.range.end()
    }

    /// Get the length of the range
    /// 
    /// 获取范围的长度
    #[inline]
    pub fn len(&self) -> u64 {
        self.range.len()
    }

    /// Check if the range is empty
    /// 
    /// 检查范围是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }
}

