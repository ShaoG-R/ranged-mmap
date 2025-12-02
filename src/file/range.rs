//! File range and write receipt types
//! 
//! 文件范围和写入凭据类型

use std::ops::Range;
use super::allocator::{align_up, align_down};

/// Result of `split_at_align_up`
/// 
/// `split_at_align_up` 的返回结果
/// 
/// # Variants
/// 
/// - `Split`: Successfully split into two non-empty ranges (low, high)
/// - `Low`: Only low range exists (split point >= end)
/// - `OutOfBounds`: Position exceeds range length (pos > len)
/// 
/// # 变体
/// 
/// - `Split`: 成功拆分为两个非空范围 (low, high)
/// - `Low`: 仅存在低范围（分割点 >= end）
/// - `OutOfBounds`: 位置超出范围长度 (pos > len)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SplitUpResult {
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
    /// Only low range exists (split point aligned to or beyond end)
    /// 
    /// 仅存在低范围（分割点对齐到 end 或超出）
    Low(AllocatedRange),
    /// Position out of bounds (pos > len)
    /// 
    /// 位置越界 (pos > len)
    OutOfBounds(AllocatedRange),
}

impl SplitUpResult {
    /// Returns true if the range was successfully split into two parts
    /// 
    /// 如果范围成功拆分为两部分则返回 true
    #[inline]
    pub fn is_split(&self) -> bool {
        matches!(self, SplitUpResult::Split { .. })
    }

    /// Returns true if position was out of bounds
    /// 
    /// 如果位置越界则返回 true
    #[inline]
    pub fn is_out_of_bounds(&self) -> bool {
        matches!(self, SplitUpResult::OutOfBounds(_))
    }

    /// Get the low range (always available except OutOfBounds)
    /// 
    /// 获取低范围（除 OutOfBounds 外始终可用）
    #[inline]
    pub fn low(&self) -> Option<AllocatedRange> {
        match self {
            SplitUpResult::Split { low, .. } => Some(*low),
            SplitUpResult::Low(range) => Some(*range),
            SplitUpResult::OutOfBounds(_) => None,
        }
    }

    /// Get the high range if split succeeded
    /// 
    /// 获取高范围（仅拆分成功时可用）
    #[inline]
    pub fn high(&self) -> Option<AllocatedRange> {
        match self {
            SplitUpResult::Split { high, .. } => Some(*high),
            _ => None,
        }
    }
}

/// Result of `split_at_align_down`
/// 
/// `split_at_align_down` 的返回结果
/// 
/// # Variants
/// 
/// - `Split`: Successfully split into two non-empty ranges (low, high)
/// - `High`: Only high range exists (split point <= start)
/// - `OutOfBounds`: Position exceeds range length (pos > len)
/// 
/// # 变体
/// 
/// - `Split`: 成功拆分为两个非空范围 (low, high)
/// - `High`: 仅存在高范围（分割点 <= start）
/// - `OutOfBounds`: 位置超出范围长度 (pos > len)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SplitDownResult {
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
    /// Only high range exists (split point aligned to or before start)
    /// 
    /// 仅存在高范围（分割点对齐到 start 或之前）
    High(AllocatedRange),
    /// Position out of bounds (pos > len)
    /// 
    /// 位置越界 (pos > len)
    OutOfBounds(AllocatedRange),
}

impl SplitDownResult {
    /// Returns true if the range was successfully split into two parts
    /// 
    /// 如果范围成功拆分为两部分则返回 true
    #[inline]
    pub fn is_split(&self) -> bool {
        matches!(self, SplitDownResult::Split { .. })
    }

    /// Returns true if position was out of bounds
    /// 
    /// 如果位置越界则返回 true
    #[inline]
    pub fn is_out_of_bounds(&self) -> bool {
        matches!(self, SplitDownResult::OutOfBounds(_))
    }

    /// Get the low range if split succeeded
    /// 
    /// 获取低范围（仅拆分成功时可用）
    #[inline]
    pub fn low(&self) -> Option<AllocatedRange> {
        match self {
            SplitDownResult::Split { low, .. } => Some(*low),
            _ => None,
        }
    }

    /// Get the high range (always available except OutOfBounds)
    /// 
    /// 获取高范围（除 OutOfBounds 外始终可用）
    #[inline]
    pub fn high(&self) -> Option<AllocatedRange> {
        match self {
            SplitDownResult::Split { high, .. } => Some(*high),
            SplitDownResult::High(range) => Some(*range),
            SplitDownResult::OutOfBounds(_) => None,
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
    /// - `SplitUpResult::Split { low, high }`: Successfully split into [start, split) and [split, end)
    /// - `SplitUpResult::Low`: Only low range exists (split point >= end)
    /// - `SplitUpResult::OutOfBounds`: Position exceeds range length (pos > len)
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。
    /// 
    /// # 返回值
    /// - `SplitUpResult::Split { low, high }`: 成功拆分为 [start, split) 和 [split, end)
    /// - `SplitUpResult::Low`: 仅存在低范围（分割点 >= end）
    /// - `SplitUpResult::OutOfBounds`: 位置超出范围长度 (pos > len)
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::{AllocatedRange, SplitUpResult};
    /// let range = AllocatedRange::from_range_unchecked(0, 8192);
    /// match range.split_at_align_up(100) {
    ///     SplitUpResult::Split { low, high } => {
    ///         assert_eq!(low.end(), 4096);  // Aligned up from 100
    ///         assert_eq!(high.start(), 4096);
    ///     }
    ///     _ => panic!("expected split"),
    /// }
    /// ```
    #[inline]
    pub fn split_at_align_up(&self, pos: u64) -> SplitUpResult {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos > len {
            return SplitUpResult::OutOfBounds(*self);
        }
        
        let split_point = align_up(start + pos);
        
        if split_point >= end {
            SplitUpResult::Low(*self)
        } else {
            SplitUpResult::Split {
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
    /// - `SplitDownResult::Split { low, high }`: Successfully split into [start, split) and [split, end)
    /// - `SplitDownResult::High`: Only high range exists (split point <= start)
    /// - `SplitDownResult::OutOfBounds`: Position exceeds range length (pos > len)
    /// 
    /// # 参数
    /// - `pos`: 从范围起始位置开始的相对偏移量。
    /// 
    /// # 返回值
    /// - `SplitDownResult::Split { low, high }`: 成功拆分为 [start, split) 和 [split, end)
    /// - `SplitDownResult::High`: 仅存在高范围（分割点 <= start）
    /// - `SplitDownResult::OutOfBounds`: 位置超出范围长度 (pos > len)
    /// 
    /// # Examples
    /// ```ignore
    /// # use ranged_mmap::file::range::{AllocatedRange, SplitDownResult};
    /// let range = AllocatedRange::from_range_unchecked(0, 8192);
    /// match range.split_at_align_down(5000) {
    ///     SplitDownResult::Split { low, high } => {
    ///         assert_eq!(low.end(), 4096);  // Aligned down from 5000
    ///         assert_eq!(high.start(), 4096);
    ///     }
    ///     _ => panic!("expected split"),
    /// }
    /// ```
    #[inline]
    pub fn split_at_align_down(&self, pos: u64) -> SplitDownResult {
        let start = self.start;
        let end = self.end;
        let len = self.len();

        if pos > len {
            return SplitDownResult::OutOfBounds(*self);
        }
        
        let split_point = align_down(start + pos);
        
        if split_point <= start {
            SplitDownResult::High(*self)
        } else {
            SplitDownResult::Split {
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
            SplitUpResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_up_already_aligned() {
        // Range [0, 8192), split at pos 4096 -> align_up(4096) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_up(ALIGNMENT) {
            SplitUpResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_up_returns_low() {
        // Range [0, 4096), split at pos 100 -> align_up(100) = 4096 >= end
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        match range.split_at_align_up(100) {
            SplitUpResult::Low(low) => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
            }
            _ => panic!("expected Low"),
        }
    }

    #[test]
    fn test_split_at_align_up_pos_beyond_len() {
        // Range [0, 4096), split at pos 5000 -> pos > len, returns OutOfBounds
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        match range.split_at_align_up(5000) {
            SplitUpResult::OutOfBounds(r) => {
                assert_eq!(r.start(), 0);
                assert_eq!(r.end(), ALIGNMENT);
            }
            _ => panic!("expected OutOfBounds"),
        }
    }

    #[test]
    fn test_split_at_align_up_pos_zero() {
        // Range [0, 8192), split at pos 0 -> align_up(0) = 0
        // split_point = 0, which is not >= end (8192), so should split with empty low
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_up(0) {
            SplitUpResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), 0);
                assert!(low.is_empty());
                assert_eq!(high.start(), 0);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected Split"),
        }
    }

    #[test]
    fn test_split_at_align_up_non_zero_start() {
        // Range [4096, 12288), split at pos 100 -> align_up(4096 + 100) = 8192
        let range = AllocatedRange::from_range_unchecked(ALIGNMENT, 3 * ALIGNMENT);
        match range.split_at_align_up(100) {
            SplitUpResult::Split { low, high } => {
                assert_eq!(low.start(), ALIGNMENT);
                assert_eq!(low.end(), 2 * ALIGNMENT);
                assert_eq!(high.start(), 2 * ALIGNMENT);
                assert_eq!(high.end(), 3 * ALIGNMENT);
            }
            _ => panic!("expected split"),
        }
    }

    // ========== split_at_align_down tests ==========

    #[test]
    fn test_split_at_align_down_basic() {
        // Range [0, 8192), split at pos 5000 -> align_down(5000) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(5000) {
            SplitDownResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_down_already_aligned() {
        // Range [0, 8192), split at pos 4096 -> align_down(4096) = 4096
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(ALIGNMENT) {
            SplitDownResult::Split { low, high } => {
                assert_eq!(low.start(), 0);
                assert_eq!(low.end(), ALIGNMENT);
                assert_eq!(high.start(), ALIGNMENT);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn test_split_at_align_down_returns_high() {
        // Range [0, 8192), split at pos 100 -> align_down(100) = 0 <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(100) {
            SplitDownResult::High(high) => {
                assert_eq!(high.start(), 0);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected High"),
        }
    }

    #[test]
    fn test_split_at_align_down_pos_beyond_len() {
        // Range [0, 4096), split at pos 5000 -> pos > len, returns OutOfBounds
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        match range.split_at_align_down(5000) {
            SplitDownResult::OutOfBounds(r) => {
                assert_eq!(r.start(), 0);
                assert_eq!(r.end(), ALIGNMENT);
            }
            _ => panic!("expected OutOfBounds"),
        }
    }

    #[test]
    fn test_split_at_align_down_pos_zero() {
        // Range [0, 8192), split at pos 0 -> align_down(0) = 0 <= start
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        match range.split_at_align_down(0) {
            SplitDownResult::High(high) => {
                assert_eq!(high.start(), 0);
                assert_eq!(high.end(), 8192);
            }
            _ => panic!("expected High"),
        }
    }

    #[test]
    fn test_split_at_align_down_non_zero_start() {
        // Range [4096, 12288), split at pos 5000 -> align_down(4096 + 5000) = 8192
        let range = AllocatedRange::from_range_unchecked(ALIGNMENT, 3 * ALIGNMENT);
        match range.split_at_align_down(5000) {
            SplitDownResult::Split { low, high } => {
                assert_eq!(low.start(), ALIGNMENT);
                assert_eq!(low.end(), 2 * ALIGNMENT);
                assert_eq!(high.start(), 2 * ALIGNMENT);
                assert_eq!(high.end(), 3 * ALIGNMENT);
            }
            _ => panic!("expected split"),
        }
    }

    // ========== Helper method tests ==========

    #[test]
    fn test_split_up_result_helpers() {
        let range = AllocatedRange::from_range_unchecked(0, 3 * ALIGNMENT);
        
        // Test with successful split
        let result = range.split_at_align_up(5000);
        assert!(result.is_split());
        assert!(!result.is_out_of_bounds());
        
        let low = result.low().unwrap();
        let high = result.high().unwrap();
        assert_eq!(low.start(), range.start());
        assert_eq!(high.end(), range.end());
        assert_eq!(low.end(), high.start());
    }

    #[test]
    fn test_split_up_result_low_helpers() {
        let range = AllocatedRange::from_range_unchecked(0, ALIGNMENT);
        
        // Test with Low (cannot split, only low range)
        let result = range.split_at_align_up(100);
        assert!(!result.is_split());
        assert!(!result.is_out_of_bounds());
        
        assert_eq!(result.low(), Some(range));
        assert_eq!(result.high(), None);
    }

    #[test]
    fn test_split_down_result_helpers() {
        let range = AllocatedRange::from_range_unchecked(0, 3 * ALIGNMENT);
        
        // Test with successful split
        let result = range.split_at_align_down(5000);
        assert!(result.is_split());
        assert!(!result.is_out_of_bounds());
        
        let low = result.low().unwrap();
        let high = result.high().unwrap();
        assert_eq!(low.start(), range.start());
        assert_eq!(high.end(), range.end());
        assert_eq!(low.end(), high.start());
    }

    #[test]
    fn test_split_down_result_high_helpers() {
        let range = AllocatedRange::from_range_unchecked(0, 8192);
        
        // Test with High (cannot split, only high range)
        let result = range.split_at_align_down(100);
        assert!(!result.is_split());
        assert!(!result.is_out_of_bounds());
        
        assert_eq!(result.low(), None);
        assert_eq!(result.high(), Some(range));
    }
}
