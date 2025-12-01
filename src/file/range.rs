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
/// # use ranged_mmap::{MmapFile, Result, allocator::ALIGNMENT};
/// # use tempfile::tempdir;
/// # fn main() -> Result<()> {
/// # let dir = tempdir()?;
/// # let path = dir.path().join("output.bin");
/// # use std::num::NonZeroU64;
/// let (file, mut allocator) = MmapFile::create_default(&path, NonZeroU64::new(ALIGNMENT).unwrap())?;
/// let range = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()).unwrap();
///
/// // Get range information (4K aligned)
/// // 获取范围信息（4K对齐）
/// assert_eq!(range.start(), 0);
/// assert_eq!(range.end(), ALIGNMENT);
/// assert_eq!(range.len(), ALIGNMENT);
///
/// let (start, end) = range.as_range_tuple();
/// assert_eq!(start, 0);
/// assert_eq!(end, ALIGNMENT);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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


    /// Split the range at the given relative position
    /// 
    /// 在给定相对位置拆分范围
    /// 
    /// # Parameters
    /// - `pos`: Relative offset from the start of the range. Must be less than the range length.
    /// 
    /// # Returns
    /// Two new ranges `(left, right)`, or an error if the position is out of bounds.
    /// - `left`: Range from `start` to `start + pos` (exclusive)
    /// - `right`: Range from `start + pos` to `end`
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。必须小于范围长度。
    /// 
    /// # 返回值
    /// 两个新的范围 `(left, right)`，如果位置超出范围则返回错误。
    /// - `left`: 从 `start` 到 `start + pos`（不包含）的范围
    /// - `right`: 从 `start + pos` 到 `end` 的范围
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::AllocatedRange;
    /// # use std::num::NonZeroU64;
    /// let range = AllocatedRange::from_range_unchecked(10, 20); // Range [10, 20)
    /// let (left, right) = range.split_at(NonZeroU64::new(5).unwrap()).unwrap();
    /// assert_eq!(left.start(), 10);
    /// assert_eq!(left.end(), 15);
    /// assert_eq!(right.start(), 15);
    /// assert_eq!(right.end(), 20);
    /// ```
    #[inline]
    pub fn split_at(&self, pos: NonZeroU64) -> Result<(AllocatedRange, AllocatedRange)> {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos.get() > len {
            return Err(Error::InvalidRange { start, end });
        }
        
        let split_point = start + pos.get();
        Ok((
            AllocatedRange::from_range_unchecked(start, split_point),
            AllocatedRange::from_range_unchecked(split_point, end)
        ))
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
/// # use ranged_mmap::{MmapFile, Result, allocator::ALIGNMENT};
/// # use tempfile::tempdir;
/// # fn main() -> Result<()> {
/// # let dir = tempdir()?;
/// # let path = dir.path().join("output.bin");
/// # use std::num::NonZeroU64;
/// let (file, mut allocator) = MmapFile::create_default(&path, NonZeroU64::new(ALIGNMENT).unwrap())?;
/// let range = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()).unwrap();
///
/// // Write and get receipt
/// // 写入并获得凭据
/// let receipt = file.write_range(range, &vec![42u8; ALIGNMENT as usize]);
///
/// // Use receipt to flush the range
/// // 使用凭据刷新该范围
/// file.flush_range(receipt)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_at_basic() {
        // Test splitting a range [0, 10) at relative position 5
        let range = AllocatedRange::from_range_unchecked(0, 10);
        let (left, right) = range.split_at(NonZeroU64::new(5).unwrap()).unwrap();
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), 5);
        assert_eq!(left.len(), 5);
        
        assert_eq!(right.start(), 5);
        assert_eq!(right.end(), 10);
        assert_eq!(right.len(), 5);
    }

    #[test]
    fn test_split_at_non_zero_start() {
        // Test splitting a range [10, 20) at relative position 5
        let range = AllocatedRange::from_range_unchecked(10, 20);
        let (left, right) = range.split_at(NonZeroU64::new(5).unwrap()).unwrap();
        
        assert_eq!(left.start(), 10);
        assert_eq!(left.end(), 15);
        assert_eq!(left.len(), 5);
        
        assert_eq!(right.start(), 15);
        assert_eq!(right.end(), 20);
        assert_eq!(right.len(), 5);
    }

    #[test]
    fn test_split_at_beginning() {
        // Test splitting at the beginning (position 1) - creates minimal left range
        let range = AllocatedRange::from_range_unchecked(0, 10);
        let (left, right) = range.split_at(NonZeroU64::new(1).unwrap()).unwrap();
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), 1);
        assert_eq!(left.len(), 1);
        
        assert_eq!(right.start(), 1);
        assert_eq!(right.end(), 10);
        assert_eq!(right.len(), 9);
    }

    #[test]
    fn test_split_at_near_end() {
        // Test splitting near the end - creates minimal right range
        let range = AllocatedRange::from_range_unchecked(0, 10);
        let (left, right) = range.split_at(NonZeroU64::new(9).unwrap()).unwrap();
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), 9);
        assert_eq!(left.len(), 9);
        
        assert_eq!(right.start(), 9);
        assert_eq!(right.end(), 10);
        assert_eq!(right.len(), 1);
    }

    #[test]
    fn test_split_at_exact_length() {
        // Test splitting at exactly the length - should create right range with zero length
        let range = AllocatedRange::from_range_unchecked(0, 10);
        let (left, right) = range.split_at(NonZeroU64::new(10).unwrap()).unwrap();
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), 10);
        assert_eq!(left.len(), 10);
        
        assert_eq!(right.start(), 10);
        assert_eq!(right.end(), 10);
        assert_eq!(right.len(), 0);
        assert!(right.is_empty());
    }

    #[test]
    fn test_split_at_beyond_length() {
        // Test splitting beyond the length - should fail
        let range = AllocatedRange::from_range_unchecked(0, 10);
        let result = range.split_at(NonZeroU64::new(11).unwrap());
        
        assert!(result.is_err());
    }

    #[test]
    fn test_split_at_large_offset() {
        // Test with large offset values
        let range = AllocatedRange::from_range_unchecked(1000, 2000);
        let (left, right) = range.split_at(NonZeroU64::new(500).unwrap()).unwrap();
        
        assert_eq!(left.start(), 1000);
        assert_eq!(left.end(), 1500);
        assert_eq!(left.len(), 500);
        
        assert_eq!(right.start(), 1500);
        assert_eq!(right.end(), 2000);
        assert_eq!(right.len(), 500);
    }

    #[test]
    fn test_split_at_preserves_total_length() {
        // Verify that the sum of split ranges equals the original range
        let range = AllocatedRange::from_range_unchecked(100, 200);
        let (left, right) = range.split_at(NonZeroU64::new(30).unwrap()).unwrap();
        
        assert_eq!(left.len() + right.len(), range.len());
        assert_eq!(left.start(), range.start());
        assert_eq!(right.end(), range.end());
        assert_eq!(left.end(), right.start());
    }

    #[test]
    fn test_split_at_edge_case_single_byte() {
        // Test with a single-byte range
        let range = AllocatedRange::from_range_unchecked(0, 1);
        let (left, right) = range.split_at(NonZeroU64::new(1).unwrap()).unwrap();
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), 1);
        assert_eq!(left.len(), 1);
        
        assert_eq!(right.start(), 1);
        assert_eq!(right.end(), 1);
        assert_eq!(right.len(), 0);
        assert!(right.is_empty());
    }

    #[test]
    fn test_split_at_multiple_splits() {
        // Test that multiple splits work correctly
        let range = AllocatedRange::from_range_unchecked(0, 100);
        
        // First split at position 25
        let (left1, right1) = range.split_at(NonZeroU64::new(25).unwrap()).unwrap();
        assert_eq!(left1.start(), 0);
        assert_eq!(left1.end(), 25);
        assert_eq!(right1.start(), 25);
        assert_eq!(right1.end(), 100);
        
        // Second split of the right part at relative position 50 (absolute 75)
        let (left2, right2) = right1.split_at(NonZeroU64::new(50).unwrap()).unwrap();
        assert_eq!(left2.start(), 25);
        assert_eq!(left2.end(), 75);
        assert_eq!(right2.start(), 75);
        assert_eq!(right2.end(), 100);
    }
}
