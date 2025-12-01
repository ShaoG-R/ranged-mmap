//! Concurrent (wait-free) range allocator implementation
//!
//! 并发（无等待）范围分配器实现

use super::{align_up, RangeAllocator};
use crate::file::range::AllocatedRange;
use std::cmp;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

/// Concurrent (wait-free) range allocator for file regions
///
/// 文件区域的并发（无等待）范围分配器
///
/// This allocator uses atomic operations to allow concurrent allocation
/// from multiple threads without locking. It is wait-free, meaning every
/// operation completes in a bounded number of steps.
///
/// 此分配器使用原子操作，允许多个线程并发分配而无需加锁。
/// 它是无等待的，意味着每个操作都在有限步骤内完成。
///
/// # Example
///
/// ```
/// use ranged_mmap::allocator::{concurrent::Allocator, RangeAllocator};
/// use std::num::NonZeroU64;
/// let allocator = Allocator::new(NonZeroU64::new(1000).unwrap());
///
/// // Concurrent allocation from multiple threads
/// // 从多个线程并发分配
/// std::thread::scope(|s| {
///     s.spawn(|| {
///         if let Some(range) = allocator.allocate(NonZeroU64::new(100).unwrap()) {
///             println!("Thread 1 got range: {:?}", range);
///         }
///     });
///     s.spawn(|| {
///         if let Some(range) = allocator.allocate(NonZeroU64::new(100).unwrap()) {
///             println!("Thread 2 got range: {:?}", range);
///         }
///     });
/// });
/// ```
pub struct Allocator {
    /// Next allocation position (atomic)
    ///
    /// 下一个分配位置（原子）
    next_pos: AtomicU64,

    /// Total file size
    ///
    /// 文件总大小
    total_size: NonZeroU64,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Allocator {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Allocator", 2)?;
        state.serialize_field("next_pos", &self.next_pos.load(Ordering::Relaxed))?;
        state.serialize_field("total_size", &self.total_size)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Allocator {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct AllocatorData {
            next_pos: u64,
            total_size: NonZeroU64,
        }
        let data = AllocatorData::deserialize(deserializer)?;
        Ok(Self {
            next_pos: AtomicU64::new(data.next_pos),
            total_size: data.total_size,
        })
    }
}

impl Allocator {
    /// Allocate a range concurrently (wait-free, 4K aligned)
    ///
    /// 并发分配范围（无等待，4K对齐）
    ///
    /// This method is safe to call from multiple threads simultaneously.
    /// The allocation size is rounded up to 4K boundary to ensure alignment.
    /// If remaining space is less than the aligned `requested_size`, allocates all
    /// remaining space.
    ///
    /// 此方法可以安全地从多个线程同时调用。
    /// 分配大小会向上对齐到4K边界以确保对齐。
    /// 如果剩余空间不足对齐后的 `requested_size`，则分配剩余的所有空间。
    ///
    /// # Parameters
    /// - `requested_size`: Number of bytes to allocate (will be aligned to 4K)
    ///
    /// # Returns
    /// Returns `Some(AllocatedRange)` on success (may be smaller than aligned request),
    /// `None` if no space is left.
    ///
    /// # 参数
    /// - `requested_size`: 要分配的字节数（会向上对齐到4K）
    ///
    /// # 返回值
    /// 成功返回 `Some(AllocatedRange)`（可能比对齐后的请求小），
    /// 没有剩余空间时返回 `None`
    ///
    /// # Note
    /// The actual allocated size may be larger than requested due to 4K alignment.
    /// For example, requesting 100 bytes will allocate 4096 bytes.
    ///
    /// # 注意
    /// 由于4K对齐，实际分配的大小可能大于请求的大小。
    /// 例如，请求100字节将分配4096字节。
    #[inline]
    pub fn allocate(&self, requested_size: NonZeroU64) -> Option<AllocatedRange> {
        // Align the requested size up to 4K boundary
        // 将请求大小向上对齐到4K边界
        let size = align_up(requested_size.get());
        let total = self.total_size.get();

        // 1. Optimistically increment counter (Wait-Free)
        // Even if this causes next_pos to exceed total_size, we handle truncation below
        // 1. 乐观地增加计数器 (Wait-Free)
        // 哪怕这会导致 next_pos 超过 total_size 也没关系，我们在后面处理截断
        let start = self.next_pos.fetch_add(size, Ordering::Relaxed);

        // 2. First check: if start position already exceeds file size,
        // space was already exhausted before this call
        // 2. 第一道检查：如果起始位置本身已经超出了文件大小
        // 说明在本次调用之前，空间就已经被分完了
        if start >= total {
            return None;
        }

        // 3. Calculate end position with clamping
        // Logic: actual end = min(theoretical end, total file size)
        // saturating_add prevents u64 overflow panic (though extremely rare)
        // 3. 计算结束位置并进行"钳位"（Clamping）
        // 逻辑：实际结束位置 = min(理论结束位置, 文件总大小)
        // saturating_add 用于防止 u64 溢出 panic（虽然极难发生）
        let theoretical_end = start.saturating_add(size);
        let end = cmp::min(theoretical_end, total);

        // At this point, end - start is the actual allocated size,
        // which may be smaller than the aligned requested_size
        // 此时，end - start 就是实际分配到的大小，它可能小于对齐后的 requested_size
        Some(AllocatedRange::from_range_unchecked(start, end))
    }
}

impl RangeAllocator for Allocator {
    #[inline]
    fn new(total_size: NonZeroU64) -> Self {
        Self {
            next_pos: AtomicU64::new(0),
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
    use std::sync::Arc;
    use std::thread;

    fn non_zero(val: u64) -> NonZeroU64 {
        NonZeroU64::new(val).unwrap()
    }

    #[test]
    fn test_concurrent_basic_allocation() {
        // Use 4K aligned total size
        let allocator = Allocator::new(non_zero(ALIGNMENT * 10)); // 40960 bytes

        // Request 100 bytes, should get 4096 (aligned up)
        let range1 = allocator.allocate(non_zero(100)).unwrap();
        assert_eq!(range1.start(), 0);
        assert_eq!(range1.end(), ALIGNMENT);
        assert_eq!(range1.len(), ALIGNMENT);
    }

    #[test]
    fn test_concurrent_multiple_allocations() {
        let allocator = Allocator::new(non_zero(ALIGNMENT * 10)); // 40960 bytes

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
    fn test_concurrent_exact_alignment() {
        let allocator = Allocator::new(non_zero(ALIGNMENT * 10));

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
    fn test_concurrent_partial_allocation() {
        // Total size: 3 * 4096 = 12288
        let allocator = Allocator::new(non_zero(ALIGNMENT * 3));

        // First allocate 2 * 4096 = 8192
        allocator.allocate(non_zero(ALIGNMENT)).unwrap();
        allocator.allocate(non_zero(ALIGNMENT)).unwrap();

        // Request more than remaining (request 8192, only 4096 left)
        let range = allocator.allocate(non_zero(ALIGNMENT * 2)).unwrap();
        assert_eq!(range.start(), ALIGNMENT * 2);
        assert_eq!(range.end(), ALIGNMENT * 3);
        assert_eq!(range.len(), ALIGNMENT);
    }

    #[test]
    fn test_concurrent_exhausted() {
        let allocator = Allocator::new(non_zero(ALIGNMENT));

        // Exhaust all space
        let range = allocator.allocate(non_zero(100)).unwrap();
        assert_eq!(range.len(), ALIGNMENT);

        // No more space
        assert!(allocator.allocate(non_zero(1)).is_none());
    }

    #[test]
    fn test_concurrent_total_size() {
        let allocator = Allocator::new(non_zero(12345));
        assert_eq!(allocator.total_size().get(), 12345);
    }

    #[test]
    fn test_concurrent_multi_thread_no_overlap() {
        // Use 4K aligned total size for clean division
        const TOTAL_SIZE: u64 = ALIGNMENT * 100; // 409600 bytes
        const NUM_THREADS: usize = 8;

        let allocator = Arc::new(Allocator::new(non_zero(TOTAL_SIZE)));
        let mut handles = Vec::new();

        for _ in 0..NUM_THREADS {
            let alloc = Arc::clone(&allocator);
            handles.push(thread::spawn(move || {
                let mut ranges = Vec::new();
                // Request 100 bytes, will allocate 4096
                while let Some(range) = alloc.allocate(non_zero(100)) {
                    ranges.push((range.start(), range.end()));
                }
                ranges
            }));
        }

        // Collect all ranges from all threads
        let mut all_ranges: Vec<(u64, u64)> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // Sort by start position
        all_ranges.sort_by_key(|r| r.0);

        // Verify no overlaps and coverage
        let mut expected_start = 0u64;
        for (start, end) in &all_ranges {
            assert!(
                *start >= expected_start,
                "Overlap detected: expected start >= {}, got {}",
                expected_start,
                start
            );
            expected_start = *end;
        }

        // All space should be allocated
        assert_eq!(expected_start, TOTAL_SIZE);
    }

    #[test]
    fn test_concurrent_stress() {
        // Use 4K aligned total size
        const TOTAL_SIZE: u64 = ALIGNMENT * 256; // 1048576 bytes (1MB)
        const NUM_THREADS: usize = 16;

        let allocator = Arc::new(Allocator::new(non_zero(TOTAL_SIZE)));
        let mut handles = Vec::new();

        for _ in 0..NUM_THREADS {
            let alloc = Arc::clone(&allocator);
            handles.push(thread::spawn(move || {
                let mut total_allocated = 0u64;
                let mut count = 0usize;
                // Request 1000 bytes, will allocate 4096
                while let Some(range) = alloc.allocate(non_zero(1000)) {
                    total_allocated += range.len();
                    count += 1;
                }
                (total_allocated, count)
            }));
        }

        let results: Vec<(u64, usize)> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let total: u64 = results.iter().map(|(t, _)| t).sum();

        // Total allocated should equal file size
        assert_eq!(total, TOTAL_SIZE);
    }

    #[test]
    fn test_concurrent_align_up_function() {
        assert_eq!(align_up(0), 0);
        assert_eq!(align_up(1), ALIGNMENT);
        assert_eq!(align_up(ALIGNMENT - 1), ALIGNMENT);
        assert_eq!(align_up(ALIGNMENT), ALIGNMENT);
        assert_eq!(align_up(ALIGNMENT + 1), ALIGNMENT * 2);
    }
}
