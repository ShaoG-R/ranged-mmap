//! Sequential range allocator implementation
//!
//! 顺序范围分配器实现

use super::{align_up, RangeAllocator};
use crate::file::range::AllocatedRange;
use std::num::NonZeroU64;

/// Sequential range allocator for file regions
///
/// 文件区域的顺序范围分配器
///
/// This allocator sequentially allocates non-overlapping ranges from the beginning
/// to the end of a file. It returns [`AllocatedRange`] types, guaranteeing that all
/// allocated ranges are valid and non-overlapping.
///
/// 此分配器从文件开头向结尾顺序分配不重叠的范围。
/// 返回 [`AllocatedRange`] 类型，保证所有分配的范围都是有效且不重叠的。
///
/// # Example
///
/// ```
/// # use ranged_mmap::allocator::{sequential::Allocator, RangeAllocator, ALIGNMENT};
/// # use std::num::NonZeroU64;
/// let mut allocator = Allocator::new(NonZeroU64::new(ALIGNMENT * 3).unwrap());
///
/// // Allocate 4K bytes (allocations are 4K aligned)
/// // 分配 4K 字节（分配是4K对齐的）
/// let range1 = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()).unwrap();
/// assert_eq!(range1.start(), 0);
/// assert_eq!(range1.end(), ALIGNMENT);
///
/// let range2 = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()).unwrap();
/// assert_eq!(range2.start(), ALIGNMENT);
/// assert_eq!(range2.end(), ALIGNMENT * 2);
///
/// // When remaining space is less than requested, allocate remaining space
/// // 当剩余空间小于请求大小时，分配剩余空间
/// let range3 = allocator.allocate(NonZeroU64::new(ALIGNMENT * 2).unwrap()).unwrap();
/// assert_eq!(range3.start(), ALIGNMENT * 2);
/// assert_eq!(range3.end(), ALIGNMENT * 3); // Only 4K bytes allocated
///
/// // Returns None when no space left
/// // 当没有剩余空间时返回 None
/// assert!(allocator.allocate(NonZeroU64::new(1).unwrap()).is_none());
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Allocator {
    /// Next allocation position
    ///
    /// 下一个分配位置
    next_pos: u64,

    /// Total file size
    ///
    /// 文件总大小
    total_size: NonZeroU64,
}

impl Allocator {
    /// Allocate a range of the specified size (4K aligned)
    ///
    /// 分配指定大小的范围（4K对齐）
    ///
    /// Allocates from the current unallocated position. The allocation size is
    /// rounded up to 4K boundary to ensure alignment. When remaining space
    /// is less than the aligned requested size, allocates all remaining space instead.
    /// Returns `None` only when no space is left.
    ///
    /// 从当前未分配位置开始分配。分配大小会向上对齐到4K边界以确保对齐。
    /// 当剩余空间小于对齐后的请求大小时，分配所有剩余空间。
    /// 仅当没有剩余空间时返回 `None`。
    ///
    /// # Note
    /// The actual allocated size may be larger than requested due to 4K alignment.
    /// For example, requesting 100 bytes will allocate 4096 bytes.
    ///
    /// # 注意
    /// 由于4K对齐，实际分配的大小可能大于请求的大小。
    /// 例如，请求100字节将分配4096字节。
    #[inline]
    pub fn allocate(&mut self, size: NonZeroU64) -> Option<AllocatedRange> {
        let remaining = self.total_size.get().saturating_sub(self.next_pos);
        if remaining == 0 {
            return None;
        }

        let start = self.next_pos;
        // Align the requested size up to 4K boundary
        // 将请求大小向上对齐到4K边界
        let aligned_size = align_up(size.get());
        // Allocate min(aligned_requested, remaining)
        let actual_size = aligned_size.min(remaining);
        let end = start + actual_size;
        self.next_pos = end;

        Some(AllocatedRange::from_range_unchecked(start, end))
    }

    /// Get the number of remaining allocatable bytes
    ///
    /// 获取剩余可分配字节数
    #[inline]
    pub fn remaining(&self) -> u64 {
        self.total_size.get().saturating_sub(self.next_pos)
    }

    /// Get the next allocation position
    ///
    /// 获取下一个分配位置
    #[inline]
    pub fn next_pos(&self) -> u64 {
        self.next_pos
    }
}

impl RangeAllocator for Allocator {
    #[inline]
    fn new(total_size: NonZeroU64) -> Self {
        Self {
            next_pos: 0,
            total_size,
        }
    }

    #[inline]
    fn total_size(&self) -> NonZeroU64 {
        self.total_size
    }
}

#[cfg(test)]
mod tests {
    use crate::allocator::ALIGNMENT;
    use super::*;

    fn non_zero(val: u64) -> NonZeroU64 {
        NonZeroU64::new(val).unwrap()
    }

    #[test]
    fn test_sequential_basic_allocation() {
        // Use 4K aligned total size
        let mut allocator = Allocator::new(non_zero(ALIGNMENT * 10)); // 40960 bytes

        // Request 100 bytes, should get 4096 (aligned up)
        let range1 = allocator.allocate(non_zero(100)).unwrap();
        assert_eq!(range1.start(), 0);
        assert_eq!(range1.end(), ALIGNMENT); // 4096
        assert_eq!(range1.len(), ALIGNMENT);
    }

    #[test]
    fn test_sequential_multiple_allocations() {
        let mut allocator = Allocator::new(non_zero(ALIGNMENT * 10)); // 40960 bytes

        // First allocation: 100 -> 4096
        let range1 = allocator.allocate(non_zero(100)).unwrap();
        assert_eq!(range1.start(), 0);
        assert_eq!(range1.end(), ALIGNMENT);

        // Second allocation: 150 -> 4096
        let range2 = allocator.allocate(non_zero(150)).unwrap();
        assert_eq!(range2.start(), ALIGNMENT);
        assert_eq!(range2.end(), ALIGNMENT * 2);

        // Third allocation: 200 -> 4096
        let range3 = allocator.allocate(non_zero(200)).unwrap();
        assert_eq!(range3.start(), ALIGNMENT * 2);
        assert_eq!(range3.end(), ALIGNMENT * 3);
    }

    #[test]
    fn test_sequential_exact_alignment() {
        let mut allocator = Allocator::new(non_zero(ALIGNMENT * 10));

        // Request exactly 4096, should get 4096
        let range = allocator.allocate(non_zero(ALIGNMENT)).unwrap();
        assert_eq!(range.start(), 0);
        assert_eq!(range.end(), ALIGNMENT);
        assert_eq!(range.len(), ALIGNMENT);

        // Request 4097, should get 8192 (aligned up)
        let range2 = allocator.allocate(non_zero(ALIGNMENT + 1)).unwrap();
        assert_eq!(range2.start(), ALIGNMENT);
        assert_eq!(range2.end(), ALIGNMENT * 3);
        assert_eq!(range2.len(), ALIGNMENT * 2);
    }

    #[test]
    fn test_sequential_partial_allocation() {
        // Total size: 3 * 4096 = 12288
        let mut allocator = Allocator::new(non_zero(ALIGNMENT * 3));

        // First allocate 2 * 4096 = 8192
        allocator.allocate(non_zero(ALIGNMENT)).unwrap(); // 4096
        allocator.allocate(non_zero(ALIGNMENT)).unwrap(); // 4096

        // Request more than remaining (request 8192, only 4096 left)
        let range = allocator.allocate(non_zero(ALIGNMENT * 2)).unwrap();
        assert_eq!(range.start(), ALIGNMENT * 2);
        assert_eq!(range.end(), ALIGNMENT * 3);
        assert_eq!(range.len(), ALIGNMENT);
    }

    #[test]
    fn test_sequential_exhausted() {
        let mut allocator = Allocator::new(non_zero(ALIGNMENT));

        // Exhaust all space
        let range = allocator.allocate(non_zero(100)).unwrap();
        assert_eq!(range.len(), ALIGNMENT);

        // No more space
        assert!(allocator.allocate(non_zero(1)).is_none());
    }

    #[test]
    fn test_sequential_remaining() {
        let mut allocator = Allocator::new(non_zero(ALIGNMENT * 3)); // 12288

        assert_eq!(allocator.remaining(), ALIGNMENT * 3);
        allocator.allocate(non_zero(100)).unwrap(); // allocates 4096
        assert_eq!(allocator.remaining(), ALIGNMENT * 2);
        allocator.allocate(non_zero(ALIGNMENT * 2)).unwrap(); // allocates remaining
        assert_eq!(allocator.remaining(), 0);
    }

    #[test]
    fn test_sequential_next_pos() {
        let mut allocator = Allocator::new(non_zero(ALIGNMENT * 10));

        assert_eq!(allocator.next_pos(), 0);
        allocator.allocate(non_zero(100)).unwrap(); // 100 -> 4096
        assert_eq!(allocator.next_pos(), ALIGNMENT);
        allocator.allocate(non_zero(250)).unwrap(); // 250 -> 4096
        assert_eq!(allocator.next_pos(), ALIGNMENT * 2);
    }

    #[test]
    fn test_sequential_total_size() {
        let allocator = Allocator::new(non_zero(12345));
        assert_eq!(allocator.total_size().get(), 12345);
    }
}
