//! File range and write receipt types
//! 
//! 文件范围和写入凭据类型

use std::ops::Range;
use super::allocator::{align_up, align_down};

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


    /// Split the range at the given relative position with 4K upper alignment
    /// 
    /// 在给定相对位置以4K上对齐方式拆分范围
    /// 
    /// # Parameters
    /// - `pos`: Relative offset from the start of the range.
    /// 
    /// # Returns
    /// `(left, Option<right>)`:
    /// - `left`: Range from `start` to `align_up(start + pos)` (4K aligned)
    /// - `right`: Range from `align_up(start + pos)` to `end`, or `None` if the aligned position >= end or pos > len
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。
    /// 
    /// # 返回值
    /// `(left, Option<right>)`：
    /// - `left`: 从 `start` 到 `align_up(start + pos)`（4K对齐）的范围
    /// - `right`: 从 `align_up(start + pos)` 到 `end` 的范围，如果对齐后的位置 >= end 或 pos > len 则为 `None`
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::AllocatedRange;
    /// let range = AllocatedRange::from_range_unchecked(0, 8192); // Range [0, 8192)
    /// let (left, right) = range.split_at_align_up(100);
    /// assert_eq!(left.start(), 0);
    /// assert_eq!(left.end(), 4096);  // Aligned up from 100
    /// assert_eq!(right.unwrap().start(), 4096);
    /// assert_eq!(right.unwrap().end(), 8192);
    /// ```
    #[inline]
    pub fn split_at_align_up(&self, pos: u64) -> (AllocatedRange, Option<AllocatedRange>) {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos > len {
            return (AllocatedRange::from_range_unchecked(start, end), None);
        }
        
        let split_point = align_up(start + pos);
        
        if split_point >= end {
            // Aligned position reaches or exceeds end, no right range
            (
                AllocatedRange::from_range_unchecked(start, end),
                None
            )
        } else {
            (
                AllocatedRange::from_range_unchecked(start, split_point),
                Some(AllocatedRange::from_range_unchecked(split_point, end))
            )
        }
    }

    /// Split the range at the given relative position with 4K lower alignment
    /// 
    /// 在给定相对位置以4K下对齐方式拆分范围
    /// 
    /// # Parameters
    /// - `pos`: Relative offset from the start of the range.
    /// 
    /// # Returns
    /// `(Option<left>, right)`:
    /// - `left`: Range from `start` to `align_down(start + pos)`, or `None` if the aligned position <= start or pos > len
    /// - `right`: Range from `align_down(start + pos)` to `end` (4K aligned)
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。
    /// 
    /// # 返回值
    /// `(Option<left>, right)`：
    /// - `left`: 从 `start` 到 `align_down(start + pos)` 的范围，如果对齐后的位置 <= start 或 pos > len 则为 `None`
    /// - `right`: 从 `align_down(start + pos)` 到 `end`（4K对齐）的范围
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::AllocatedRange;
    /// let range = AllocatedRange::from_range_unchecked(0, 8192); // Range [0, 8192)
    /// let (left, right) = range.split_at_align_down(5000);
    /// assert_eq!(left.unwrap().start(), 0);
    /// assert_eq!(left.unwrap().end(), 4096);  // Aligned down from 5000
    /// assert_eq!(right.start(), 4096);
    /// assert_eq!(right.end(), 8192);
    /// ```
    #[inline]
    pub fn split_at_align_down(&self, pos: u64) -> (Option<AllocatedRange>, AllocatedRange) {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos > len {
            return (None, AllocatedRange::from_range_unchecked(start, end));
        }
        
        let split_point = align_down(start + pos);
        
        if split_point <= start {
            // Aligned position is at or before start, no left range
            (
                None,
                AllocatedRange::from_range_unchecked(start, end)
            )
        } else {
            (
                Some(AllocatedRange::from_range_unchecked(start, split_point)),
                AllocatedRange::from_range_unchecked(split_point, end)
            )
        }
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
    use super::super::allocator::ALIGNMENT;

    // ========== split_at_align_up tests ==========

    #[test]
    fn test_split_at_align_up_basic() {
        // Range [0, 8192), split at pos 100 -> align_up(100) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_up(100);
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), ALIGNMENT);
        
        let right = right.unwrap();
        assert_eq!(right.start(), ALIGNMENT);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_up_already_aligned() {
        // Range [0, 8192), split at pos 4096 -> align_up(4096) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_up(ALIGNMENT);
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), ALIGNMENT);
        
        let right = right.unwrap();
        assert_eq!(right.start(), ALIGNMENT);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_up_no_right_range() {
        // Range [0, 4096), split at pos 100 -> align_up(100) = 4096 >= end
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        let (left, right) = range.split_at_align_up(100);
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), ALIGNMENT);
        assert!(right.is_none());
    }

    #[test]
    fn test_split_at_align_up_pos_beyond_len() {
        // Range [0, 4096), split at pos 5000 -> pos > len, returns full range
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        let (left, right) = range.split_at_align_up(5000);
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), ALIGNMENT);
        assert!(right.is_none());
    }

    #[test]
    fn test_split_at_align_up_pos_zero() {
        // Range [0, 8192), split at pos 0 -> align_up(0) = 0
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_up(0);
        
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), 0);
        assert!(left.is_empty());
        
        let right = right.unwrap();
        assert_eq!(right.start(), 0);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_up_non_zero_start() {
        // Range [4096, 12288), split at pos 100 -> align_up(4096 + 100) = 8192
        let range = AllocatedRange::from_range_unchecked(ALIGNMENT, 3 * ALIGNMENT);
        let (left, right) = range.split_at_align_up(100);
        
        assert_eq!(left.start(), ALIGNMENT);
        assert_eq!(left.end(), 2 * ALIGNMENT);
        
        let right = right.unwrap();
        assert_eq!(right.start(), 2 * ALIGNMENT);
        assert_eq!(right.end(), 3 * ALIGNMENT);
    }

    // ========== split_at_align_down tests ==========

    #[test]
    fn test_split_at_align_down_basic() {
        // Range [0, 8192), split at pos 5000 -> align_down(5000) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_down(5000);
        
        let left = left.unwrap();
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), ALIGNMENT);
        
        assert_eq!(right.start(), ALIGNMENT);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_down_already_aligned() {
        // Range [0, 8192), split at pos 4096 -> align_down(4096) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_down(ALIGNMENT);
        
        let left = left.unwrap();
        assert_eq!(left.start(), 0);
        assert_eq!(left.end(), ALIGNMENT);
        
        assert_eq!(right.start(), ALIGNMENT);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_down_no_left_range() {
        // Range [0, 8192), split at pos 100 -> align_down(100) = 0 <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_down(100);
        
        assert!(left.is_none());
        assert_eq!(right.start(), 0);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_down_pos_beyond_len() {
        // Range [0, 4096), split at pos 5000 -> pos > len, returns full range
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        let (left, right) = range.split_at_align_down(5000);
        
        assert!(left.is_none());
        assert_eq!(right.start(), 0);
        assert_eq!(right.end(), ALIGNMENT);
    }

    #[test]
    fn test_split_at_align_down_pos_zero() {
        // Range [0, 8192), split at pos 0 -> align_down(0) = 0 <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        let (left, right) = range.split_at_align_down(0);
        
        assert!(left.is_none());
        assert_eq!(right.start(), 0);
        assert_eq!(right.end(), 8192);
    }

    #[test]
    fn test_split_at_align_down_non_zero_start() {
        // Range [4096, 12288), split at pos 5000 -> align_down(4096 + 5000) = 8192
        let range = AllocatedRange::from_range_unchecked(ALIGNMENT, 3 * ALIGNMENT);
        let (left, right) = range.split_at_align_down(5000);
        
        let left = left.unwrap();
        assert_eq!(left.start(), ALIGNMENT);
        assert_eq!(left.end(), 2 * ALIGNMENT);
        
        assert_eq!(right.start(), 2 * ALIGNMENT);
        assert_eq!(right.end(), 3 * ALIGNMENT);
    }

    #[test]
    fn test_split_preserves_total_coverage() {
        // Verify that split ranges cover the original range
        let range = AllocatedRange::from_range_unchecked(0, 3 * ALIGNMENT);
        
        let (left, right) = range.split_at_align_up(5000);
        let right = right.unwrap();
        assert_eq!(left.start(), range.start());
        assert_eq!(right.end(), range.end());
        assert_eq!(left.end(), right.start());
    }
}
