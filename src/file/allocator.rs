//! Range allocator implementation
//!
//! Range 分配器实现

pub mod concurrent;
pub mod sequential;

use std::num::NonZeroU64;

/// 4K alignment size in bytes (4096 = 0x1000)
///
/// 4K对齐大小（字节）
pub const ALIGNMENT: u64 = 4096;

/// Align a value up to the nearest 4K boundary
///
/// 将值向上对齐到最近的4K边界
///
/// # Examples
///
/// ```
/// # use ranged_mmap::allocator::align_up;
/// assert_eq!(align_up(0), 0);
/// assert_eq!(align_up(1), 4096);
/// assert_eq!(align_up(4096), 4096);
/// assert_eq!(align_up(4097), 8192);
/// ```
#[inline]
pub const fn align_up(value: u64) -> u64 {
    // (value + ALIGNMENT - 1) & !(ALIGNMENT - 1)
    // Equivalent but handles overflow better
    match value % ALIGNMENT {
        0 => value,
        remainder => value + (ALIGNMENT - remainder),
    }
}

/// Align a value down to the nearest 4K boundary
///
/// 将值向下对齐到最近的4K边界
///
/// # Examples
///
/// ```
/// # use ranged_mmap::allocator::align_down;
/// assert_eq!(align_down(0), 0);
/// assert_eq!(align_down(1), 0);
/// assert_eq!(align_down(4095), 0);
/// assert_eq!(align_down(4096), 4096);
/// assert_eq!(align_down(4097), 4096);
/// assert_eq!(align_down(8192), 8192);
/// ```
#[inline]
pub const fn align_down(value: u64) -> u64 {
    value & !(ALIGNMENT - 1)
}

/// Trait for range allocators
///
/// 范围分配器 trait
///
/// This trait defines the interface for allocating non-overlapping ranges
/// from a file. Implementations must guarantee that all allocated ranges
/// are valid and non-overlapping.
///
/// 此 trait 定义了从文件中分配不重叠范围的接口。
/// 实现必须保证所有分配的范围都是有效且不重叠的。
pub trait RangeAllocator: Sized {
    /// Create a new range allocator
    ///
    /// 创建新的范围分配器
    ///
    /// # Parameters
    /// - `total_size`: Total file size in bytes
    ///
    /// # 参数
    /// - `total_size`: 文件总大小（字节）
    fn new(total_size: NonZeroU64) -> Self;

    /// Get the total size
    ///
    /// 获取总大小
    fn total_size(&self) -> NonZeroU64;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== align_up tests ==========

    #[test]
    fn test_align_up_zero() {
        assert_eq!(align_up(0), 0);
    }

    #[test]
    fn test_align_up_already_aligned() {
        assert_eq!(align_up(ALIGNMENT), ALIGNMENT);
        assert_eq!(align_up(2 * ALIGNMENT), 2 * ALIGNMENT);
        assert_eq!(align_up(100 * ALIGNMENT), 100 * ALIGNMENT);
    }

    #[test]
    fn test_align_up_not_aligned() {
        assert_eq!(align_up(1), ALIGNMENT);
        assert_eq!(align_up(ALIGNMENT - 1), ALIGNMENT);
        assert_eq!(align_up(ALIGNMENT + 1), 2 * ALIGNMENT);
        assert_eq!(align_up(2 * ALIGNMENT - 1), 2 * ALIGNMENT);
    }

    #[test]
    fn test_align_up_mid_values() {
        assert_eq!(align_up(2048), ALIGNMENT);
        assert_eq!(align_up(ALIGNMENT + 2048), 2 * ALIGNMENT);
    }

    #[test]
    fn test_align_up_large_values() {
        let large = 1_000_000_000u64;
        let aligned = align_up(large);
        assert!(aligned >= large);
        assert_eq!(aligned % ALIGNMENT, 0);
    }

    // ========== align_down tests ==========

    #[test]
    fn test_align_down_zero() {
        assert_eq!(align_down(0), 0);
    }

    #[test]
    fn test_align_down_already_aligned() {
        assert_eq!(align_down(ALIGNMENT), ALIGNMENT);
        assert_eq!(align_down(2 * ALIGNMENT), 2 * ALIGNMENT);
        assert_eq!(align_down(100 * ALIGNMENT), 100 * ALIGNMENT);
    }

    #[test]
    fn test_align_down_not_aligned() {
        assert_eq!(align_down(1), 0);
        assert_eq!(align_down(ALIGNMENT - 1), 0);
        assert_eq!(align_down(ALIGNMENT + 1), ALIGNMENT);
        assert_eq!(align_down(2 * ALIGNMENT - 1), ALIGNMENT);
    }

    #[test]
    fn test_align_down_mid_values() {
        assert_eq!(align_down(2048), 0);
        assert_eq!(align_down(ALIGNMENT + 2048), ALIGNMENT);
    }

    #[test]
    fn test_align_down_large_values() {
        let large = 1_000_000_000u64;
        let aligned = align_down(large);
        assert!(aligned <= large);
        assert_eq!(aligned % ALIGNMENT, 0);
    }

    // ========== align_up and align_down relationship tests ==========

    #[test]
    fn test_align_up_down_relationship() {
        // For aligned values, both should return the same
        assert_eq!(align_up(ALIGNMENT), align_down(ALIGNMENT));
        
        // For non-aligned values, align_up > align_down
        for i in 1..ALIGNMENT {
            assert!(align_up(i) > align_down(i));
            assert_eq!(align_up(i) - align_down(i), ALIGNMENT);
        }
    }

    #[test]
    fn test_align_round_trip() {
        // align_down(align_up(x)) == align_up(x) for all x
        for x in [0, 1, 100, ALIGNMENT - 1, ALIGNMENT, ALIGNMENT + 1, 10000] {
            let up = align_up(x);
            assert_eq!(align_down(up), up);
        }
    }
}
