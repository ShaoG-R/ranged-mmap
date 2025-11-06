//! Range allocator implementation
//! 
//! Range 分配器实现

use super::range::AllocatedRange;

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
/// ```ignore
/// # use ranged_mmap::RangeAllocator;
/// let mut allocator = RangeAllocator::new(1000);
///
/// // Allocate 100 bytes
/// // 分配 100 字节
/// let range1 = allocator.allocate(100).unwrap();
/// assert_eq!(range1.start(), 0);
/// assert_eq!(range1.end(), 100);
///
/// let range2 = allocator.allocate(150).unwrap();
/// assert_eq!(range2.start(), 100);
/// assert_eq!(range2.end(), 250);
///
/// assert_eq!(allocator.remaining(), 750);
/// ```
pub struct RangeAllocator {
    /// Next allocation position
    /// 
    /// 下一个分配位置
    next_pos: u64,
    
    /// Total file size
    /// 
    /// 文件总大小
    total_size: u64,
}

impl RangeAllocator {
    /// Create a new range allocator
    /// 
    /// 创建新的范围分配器
    /// 
    /// # Parameters
    /// - `total_size`: Total file size in bytes
    /// 
    /// # 参数
    /// - `total_size`: 文件总大小（字节）
    #[inline]
    pub(crate) fn new(total_size: u64) -> Self {
        Self {
            next_pos: 0,
            total_size,
        }
    }

    /// Allocate a range of the specified size
    /// 
    /// 分配指定大小的范围
    /// 
    /// Allocates from the current unallocated position. Returns `None` if
    /// insufficient space is available.
    /// 
    /// 从当前未分配位置开始分配。如果空间不足则返回 `None`。
    /// 
    /// # Parameters
    /// - `size`: Number of bytes to allocate
    /// 
    /// # Returns
    /// Returns `Some(AllocatedRange)` on success, `None` if insufficient space
    /// 
    /// # 参数
    /// - `size`: 要分配的字节数
    /// 
    /// # 返回值
    /// 成功返回 `Some(AllocatedRange)`，空间不足返回 `None`
    #[inline]
    pub fn allocate(&mut self, size: u64) -> Option<AllocatedRange> {
        if self.next_pos + size > self.total_size {
            return None;
        }

        let start = self.next_pos;
        let end = start + size;
        self.next_pos = end;

        Some(AllocatedRange::from_range(start, end))
    }

    /// Get the number of remaining allocatable bytes
    /// 
    /// 获取剩余可分配字节数
    /// 
    /// # Returns
    /// Number of bytes not yet allocated
    /// 
    /// # 返回值
    /// 返回还未分配的字节数
    #[inline]
    pub fn remaining(&self) -> u64 {
        self.total_size.saturating_sub(self.next_pos)
    }

    /// Get the total size
    /// 
    /// 获取总大小
    #[inline]
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Get the next allocation position
    /// 
    /// 获取下一个分配位置
    #[inline]
    pub fn next_pos(&self) -> u64 {
        self.next_pos
    }
}

