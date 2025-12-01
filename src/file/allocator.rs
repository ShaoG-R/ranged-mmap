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

