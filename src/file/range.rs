//! File range and write receipt types
//! 
//! 文件范围和写入凭据类型

use std::ops::Range;
use super::allocator::{align_up, align_down};

/// Result of splitting an allocated range at a 4K-aligned position
/// 
/// 在4K对齐位置拆分已分配范围的结果
/// 
/// # Variants
/// 
/// - `Split`: Successfully split into two non-empty ranges (low, high)
/// - `Whole`: Cannot split, returns the original range
/// 
/// # 变体
/// 
/// - `Split`: 成功拆分为两个非空范围 (low, high)
/// - `Whole`: 无法拆分，返回原始范围
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SplitResult {
    /// Successfully split into two ranges
    /// 
    /// 成功拆分为两个范围
    Split {
        /// Lower range [start, split_point)
        /// 
        /// 低范围 [start, split_point)
        low: AllocatedRange,
        /// Higher range [split_point, end)
        /// 
        /// 高范围 [split_point, end)
        high: AllocatedRange,
    },
    /// Cannot split, returns the whole range
    /// 
    /// 无法拆分，返回完整范围
    Whole(AllocatedRange),
}

impl SplitResult {
    /// Returns true if the range was successfully split
    /// 
    /// 如果范围成功拆分则返回 true
    #[inline]
    pub fn is_split(&self) -> bool {
        matches!(self, SplitResult::Split { .. })
    }

    /// Returns true if the range could not be split
    /// 
    /// 如果范围无法拆分则返回 true
    #[inline]
    pub fn is_whole(&self) -> bool {
        matches!(self, SplitResult::Whole(_))
    }

    /// Get the low range if split, or the whole range
    /// 
    /// 获取低范围（如果已拆分）或完整范围
    #[inline]
    pub fn low_or_whole(&self) -> AllocatedRange {
        match self {
            SplitResult::Split { low, .. } => *low,
            SplitResult::Whole(range) => *range,
        }
    }

    /// Get the high range if split, or the whole range
    /// 
    /// 获取高范围（如果已拆分）或完整范围
    #[inline]
    pub fn high_or_whole(&self) -> AllocatedRange {
        match self {
            SplitResult::Split { high, .. } => *high,
            SplitResult::Whole(range) => *range,
        }
    }

    /// Get both ranges as a tuple (low, high), or (whole, whole) if not split
    /// 
    /// 获取两个范围的元组 (low, high)，如果未拆分则返回 (whole, whole)
    #[inline]
    pub fn as_tuple(&self) -> (AllocatedRange, AllocatedRange) {
        match self {
            SplitResult::Split { low, high } => (*low, *high),
            SplitResult::Whole(range) => (*range, *range),
        }
    }
}

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
    /// The split point is calculated as `align_up(start + pos)`.
    /// 
    /// 分割点计算为 `align_up(start + pos)`。
    /// 
    /// # Parameters
    /// - `pos`: Relative offset from the start of the range.
    /// 
    /// # Returns
    /// - `SplitResult::Split { low, high }`: Successfully split into [start, split) and [split, end)
    /// - `SplitResult::Whole`: Cannot split (pos > len, or aligned position >= end)
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。
    /// 
    /// # 返回值
    /// - `SplitResult::Split { low, high }`: 成功拆分为 [start, split) 和 [split, end)
    /// - `SplitResult::Whole`: 无法拆分（pos > len，或对齐后位置 >= end）
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::{AllocatedRange, SplitResult};
    /// let range = AllocatedRange::from_range_unchecked(0, 8192);
    /// match range.split_at_align_up(100) {
    ///     SplitResult::Split { low, high } => {
    ///         assert_eq!(low.end(), 4096);  // Aligned up from 100
    ///         assert_eq!(high.start(), 4096);
    ///     }
    ///     SplitResult::Whole(_) => panic!("expected split"),
    /// }
    /// ```
    #[inline]
    pub fn split_at_align_up(&self, pos: u64) -> SplitResult {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos > len {
            return SplitResult::Whole(*self);
        }
        
        let split_point = align_up(start + pos);
        
        if split_point >= end || split_point <= start {
            SplitResult::Whole(*self)
        } else {
            SplitResult::Split {
                low: AllocatedRange::from_range_unchecked(start, split_point),
                high: AllocatedRange::from_range_unchecked(split_point, end),
            }
        }
    }

    /// Split the range at the given relative position with 4K lower alignment
    /// 
    /// 在给定相对位置以4K下对齐方式拆分范围
    /// 
    /// The split point is calculated as `align_down(start + pos)`.
    /// 
    /// 分割点计算为 `align_down(start + pos)`。
    /// 
    /// # Parameters
    /// - `pos`: Relative offset from the start of the range.
    /// 
    /// # Returns
    /// - `SplitResult::Split { low, high }`: Successfully split into [start, split) and [split, end)
    /// - `SplitResult::Whole`: Cannot split (pos > len, or aligned position <= start)
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。
    /// 
    /// # 返回值
    /// - `SplitResult::Split { low, high }`: 成功拆分为 [start, split) 和 [split, end)
    /// - `SplitResult::Whole`: 无法拆分（pos > len，或对齐后位置 <= start）
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::{AllocatedRange, SplitResult};
    /// let range = AllocatedRange::from_range_unchecked(0, 8192);
    /// match range.split_at_align_down(5000) {
    ///     SplitResult::Split { low, high } => {
    ///         assert_eq!(low.end(), 4096);  // Aligned down from 5000
    ///         assert_eq!(high.start(), 4096);
    ///     }
    ///     SplitResult::Whole(_) => panic!("expected split"),
    /// }
    /// ```
    #[inline]
    pub fn split_at_align_down(&self, pos: u64) -> SplitResult {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos > len {
            return SplitResult::Whole(*self);
        }
        
        let split_point = align_down(start + pos);
        
        if split_point <= start || split_point >= end {
            SplitResult::Whole(*self)
        } else {
            SplitResult::Split {
                low: AllocatedRange::from_range_unchecked(start, split_point),
                high: AllocatedRange::from_range_unchecked(split_point, end),
            }
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
        match range.split_at_align_up(100) {
            SplitResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            SplitResult::Whole(_) => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_up_already_aligned() {
        // Range [0, 8192), split at pos 4096 -> align_up(4096) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_up(ALIGNMENT) {
            SplitResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            SplitResult::Whole(_) => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_up_returns_whole() {
        // Range [0, 4096), split at pos 100 -> align_up(100) = 4096 >= end
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        match range.split_at_align_up(100) {
            SplitResult::Whole(whole) => {
                assert_eq!(whole.start(), 0);
                assert_eq!(whole.end(), ALIGNMENT);
            }
            SplitResult::Split { .. } => panic!("expected whole"),
        }
    }

    #[test]
    fn test_split_at_align_up_pos_beyond_len() {
        // Range [0, 4096), split at pos 5000 -> pos > len, returns whole
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        match range.split_at_align_up(5000) {
            SplitResult::Whole(whole) => {
                assert_eq!(whole.start(), 0);
                assert_eq!(whole.end(), ALIGNMENT);
            }
            SplitResult::Split { .. } => panic!("expected whole"),
        }
    }

    #[test]
    fn test_split_at_align_up_pos_zero() {
        // Range [0, 8192), split at pos 0 -> align_up(0) = 0, split_point <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_up(0) {
            SplitResult::Whole(whole) => {
                assert_eq!(whole.start(), 0);
                assert_eq!(whole.end(), 8192);
            }
            SplitResult::Split { .. } => panic!("expected whole"),
        }
    }

    #[test]
    fn test_split_at_align_up_non_zero_start() {
        // Range [4096, 12288), split at pos 100 -> align_up(4096 + 100) = 8192
        let range = AllocatedRange::from_range_unchecked(ALIGNMENT, 3 * ALIGNMENT);
        match range.split_at_align_up(100) {
            SplitResult::Split { low, high } => {
                assert_eq!(low.start(), ALIGNMENT);
                assert_eq!(low.end(), 2 * ALIGNMENT);
                assert_eq!(high.start(), 2 * ALIGNMENT);
                assert_eq!(high.end(), 3 * ALIGNMENT);
            }
            SplitResult::Whole(_) => panic!("expected split"),
        }
    }

    // ========== split_at_align_down tests ==========

    #[test]
    fn test_split_at_align_down_basic() {
        // Range [0, 8192), split at pos 5000 -> align_down(5000) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(5000) {
            SplitResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            SplitResult::Whole(_) => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_down_already_aligned() {
        // Range [0, 8192), split at pos 4096 -> align_down(4096) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(ALIGNMENT) {
            SplitResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            SplitResult::Whole(_) => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_down_returns_whole() {
        // Range [0, 8192), split at pos 100 -> align_down(100) = 0 <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(100) {
            SplitResult::Whole(whole) => {
                assert_eq!(whole.start(), 0);
                assert_eq!(whole.end(), 8192);
            }
            SplitResult::Split { .. } => panic!("expected whole"),
        }
    }

    #[test]
    fn test_split_at_align_down_pos_beyond_len() {
        // Range [0, 4096), split at pos 5000 -> pos > len, returns whole
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        match range.split_at_align_down(5000) {
            SplitResult::Whole(whole) => {
                assert_eq!(whole.start(), 0);
                assert_eq!(whole.end(), ALIGNMENT);
            }
            SplitResult::Split { .. } => panic!("expected whole"),
        }
    }

    #[test]
    fn test_split_at_align_down_pos_zero() {
        // Range [0, 8192), split at pos 0 -> align_down(0) = 0 <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(0) {
            SplitResult::Whole(whole) => {
                assert_eq!(whole.start(), 0);
                assert_eq!(whole.end(), 8192);
            }
            SplitResult::Split { .. } => panic!("expected whole"),
        }
    }

    #[test]
    fn test_split_at_align_down_non_zero_start() {
        // Range [4096, 12288), split at pos 5000 -> align_down(4096 + 5000) = 8192
        let range = AllocatedRange::from_range_unchecked(ALIGNMENT, 3 * ALIGNMENT);
        match range.split_at_align_down(5000) {
            SplitResult::Split { low, high } => {
                assert_eq!(low.start(), ALIGNMENT);
                assert_eq!(low.end(), 2 * ALIGNMENT);
                assert_eq!(high.start(), 2 * ALIGNMENT);
                assert_eq!(high.end(), 3 * ALIGNMENT);
            }
            SplitResult::Whole(_) => panic!("expected split"),
        }
    }

    // ========== SplitResult helper tests ==========

    #[test]
    fn test_split_result_helpers() {
        let range = AllocatedRange::from_range_unchecked(0, 3 * ALIGNMENT);
        
        // Test with successful split
        let result = range.split_at_align_up(5000);
        assert!(result.is_split());
        assert!(!result.is_whole());
        
        let (low, high) = result.as_tuple();
        assert_eq!(low.start(), range.start());
        assert_eq!(high.end(), range.end());
        assert_eq!(low.end(), high.start());
    }

    #[test]
    fn test_split_result_whole_helpers() {
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        
        // Test with whole (cannot split)
        let result = range.split_at_align_up(100);
        assert!(!result.is_split());
        assert!(result.is_whole());
        
        assert_eq!(result.low_or_whole(), range);
        assert_eq!(result.high_or_whole(), range);
    }
}
