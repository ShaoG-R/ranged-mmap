//! 测试模块

use super::*;
use tempfile::tempdir;

/// MmapFileInner 测试（Unsafe 版本）
#[cfg(test)]
mod mmap_file_inner_tests {
    use super::*;

    #[test]
    fn test_create_and_basic_operations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_basic.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();
        assert_eq!(file.size(), 100);

        // 基本写入操作
        unsafe {
            file.write_all_at(0, b"hello").unwrap();
            file.write_all_at(50, b"world").unwrap();
            file.sync_all().unwrap();
        }

        // 验证读取
        let mut buf = vec![0u8; 100];
        unsafe {
            file.read_at(0, &mut buf).unwrap();
        }

        assert_eq!(&buf[0..5], b"hello");
        assert_eq!(&buf[50..55], b"world");
    }

    #[test]
    fn test_open_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_open.bin");

        // 先创建文件
        let file1 = MmapFileInner::create(&path, 100).unwrap();
        unsafe {
            file1.write_all_at(0, b"test").unwrap();
            file1.sync_all().unwrap();
        }
        drop(file1);

        // 重新打开
        let file2 = MmapFileInner::open(&path).unwrap();
        assert_eq!(file2.size(), 100);

        let mut buf = vec![0u8; 4];
        unsafe {
            file2.read_at(0, &mut buf).unwrap();
        }
        assert_eq!(&buf, b"test");
    }

    #[test]
    fn test_write_at_returns_correct_length() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_write_len.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();

        unsafe {
            let written = file.write_at(0, b"hello").unwrap();
            assert_eq!(written, 5);

            let written = file.write_at(10, b"world").unwrap();
            assert_eq!(written, 5);
        }
    }

    #[test]
    fn test_concurrent_non_overlapping_writes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_concurrent.bin");

        let file = MmapFileInner::create(&path, 1000).unwrap();

        // 10个线程并发写入不重叠区域
        std::thread::scope(|s| {
            for i in 0..10 {
                let f = file.clone();
                s.spawn(move || {
                    let data = vec![i as u8; 100];
                    unsafe {
                        f.write_all_at(i * 100, &data).unwrap();
                    }
                });
            }
        });

        unsafe { file.sync_all().unwrap(); }

        // 验证每个区域的数据正确
        for i in 0..10u64 {
            let mut buf = vec![0u8; 100];
            unsafe {
                file.read_at(i * 100, &mut buf).unwrap();
            }
            assert_eq!(buf, vec![i as u8; 100]);
        }
    }

    #[test]
    fn test_high_concurrency() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_high_concurrency.bin");

        // 100个并发写入
        let num_threads = 100;
        let chunk_size = 1024;
        let file_size = num_threads * chunk_size;

        let file = MmapFileInner::create(&path, file_size as u64).unwrap();

        std::thread::scope(|s| {
            for i in 0..num_threads {
                let f = file.clone();
                s.spawn(move || {
                    let data = vec![i as u8; chunk_size];
                    unsafe {
                        f.write_all_at((i * chunk_size) as u64, &data).unwrap();
                    }
                });
            }
        });

        unsafe {
            file.sync_all().unwrap();

            // 验证
            for i in 0..num_threads {
                let mut buf = vec![0u8; chunk_size];
                file.read_at((i * chunk_size) as u64, &mut buf).unwrap();
                assert_eq!(buf, vec![i as u8; chunk_size]);
            }
        }
    }

    #[test]
    fn test_out_of_order_writes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_out_of_order.bin");

        let file = MmapFileInner::create(&path, 300).unwrap();

        // 乱序写入
        unsafe {
            file.write_all_at(200, b"third").unwrap();
            file.write_all_at(0, b"first").unwrap();
            file.write_all_at(100, b"second").unwrap();
            file.sync_all().unwrap();
        }

        // 验证
        let mut buf1 = vec![0u8; 5];
        let mut buf2 = vec![0u8; 6];
        let mut buf3 = vec![0u8; 5];

        unsafe {
            file.read_at(0, &mut buf1).unwrap();
            file.read_at(100, &mut buf2).unwrap();
            file.read_at(200, &mut buf3).unwrap();
        }

        assert_eq!(&buf1, b"first");
        assert_eq!(&buf2, b"second");
        assert_eq!(&buf3, b"third");
    }

    #[test]
    fn test_large_file_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_large.bin");

        // 写入 10MB
        let size = 10 * 1024 * 1024;
        let file = MmapFileInner::create(&path, size).unwrap();

        let data = vec![0xAB; size as usize];
        unsafe {
            file.write_all_at(0, &data).unwrap();
            file.sync_all().unwrap();
        }

        assert_eq!(file.size(), size);

        // 验证部分数据
        let mut buf = vec![0u8; 1024];
        unsafe {
            file.read_at(0, &mut buf).unwrap();
            assert!(buf.iter().all(|&b| b == 0xAB));

            file.read_at(size - 1024, &mut buf).unwrap();
            assert!(buf.iter().all(|&b| b == 0xAB));
        }
    }

    #[test]
    fn test_bounds_checking() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_bounds.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();

        // 尝试写入超出边界
        let result = unsafe { file.write_at(95, b"hello world") };
        assert!(result.is_err());

        // 刚好在边界
        let result = unsafe { file.write_at(95, b"hello") };
        assert!(result.is_ok());

        // 完全超出边界
        let result = unsafe { file.write_at(100, b"x") };
        assert!(result.is_err());

        // offset 本身就超出边界
        let result = unsafe { file.write_at(200, b"x") };
        assert!(result.is_err());
    }

    #[test]
    fn test_read_at_bounds() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_read_bounds.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();

        unsafe {
            file.write_all_at(0, b"hello").unwrap();
        }

        // 正常读取
        let mut buf = vec![0u8; 5];
        unsafe {
            let n = file.read_at(0, &mut buf).unwrap();
            assert_eq!(n, 5);
            assert_eq!(&buf, b"hello");
        }

        // 读取超出边界，应返回部分数据
        let mut buf = vec![0u8; 50];
        unsafe {
            let n = file.read_at(90, &mut buf).unwrap();
            assert_eq!(n, 10); // 只能读取 10 字节
        }

        // 完全超出边界
        let mut buf = vec![0u8; 10];
        unsafe {
            let n = file.read_at(100, &mut buf).unwrap();
            assert_eq!(n, 0); // 返回 0
        }
    }

    #[test]
    fn test_fill_operations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_fill.bin");

        let file = MmapFileInner::create(&path, 1000).unwrap();

        // 填充为 0xFF
        unsafe {
            file.fill(0xFF).unwrap();
            file.sync_all().unwrap();

            let mut buf = vec![0u8; 1000];
            file.read_at(0, &mut buf).unwrap();
            assert_eq!(buf, vec![0xFF; 1000]);
        }

        // 清零
        unsafe {
            file.zero().unwrap();
            file.sync_all().unwrap();

            let mut buf = vec![0u8; 1000];
            file.read_at(0, &mut buf).unwrap();
            assert_eq!(buf, vec![0x00; 1000]);
        }
    }

    #[test]
    fn test_read_slice() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_read_slice.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();

        unsafe {
            file.write_all_at(10, b"hello world").unwrap();
            
            let slice = file.read_slice(10, 11).unwrap();
            assert_eq!(slice, b"hello world");

            let slice = file.read_slice(10, 5).unwrap();
            assert_eq!(slice, b"hello");
        }
    }

    #[test]
    fn test_flush_range() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_flush_range.bin");

        let file = MmapFileInner::create(&path, 1000).unwrap();

        unsafe {
            file.write_all_at(0, b"hello").unwrap();
            file.write_all_at(500, b"world").unwrap();

            // 刷新特定区域
            file.flush_range(0, 5).unwrap();
            file.flush_range(500, 5).unwrap();

            // 刷新超出边界应该失败
            let result = file.flush_range(990, 20);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_zero_size_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_zero_size.bin");

        let result = MmapFileInner::create(&path, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_clone_and_shared_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_clone.bin");

        let file1 = MmapFileInner::create(&path, 100).unwrap();
        let file2 = file1.clone();

        // 两个引用写入不同位置
        unsafe {
            file1.write_all_at(0, b"file1").unwrap();
            file2.write_all_at(50, b"file2").unwrap();
            file1.sync_all().unwrap();
        }

        // 从任一引用读取都能看到所有写入
        let mut buf = vec![0u8; 100];
        unsafe {
            file2.read_at(0, &mut buf).unwrap();
        }
        assert_eq!(&buf[0..5], b"file1");
        assert_eq!(&buf[50..55], b"file2");
    }

    #[test]
    fn test_as_ptr() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_as_ptr.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();

        unsafe {
            file.write_all_at(0, b"hello").unwrap();

            let ptr = file.as_ptr();
            assert!(!ptr.is_null());

            // 通过原始指针读取
            let slice = std::slice::from_raw_parts(ptr, 5);
            assert_eq!(slice, b"hello");
        }
    }

    #[test]
    fn test_as_mut_ptr() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inner_as_mut_ptr.bin");

        let file = MmapFileInner::create(&path, 100).unwrap();

        unsafe {
            let mut_ptr = file.as_mut_ptr();
            assert!(!mut_ptr.is_null());

            // 通过原始指针写入
            let slice = std::slice::from_raw_parts_mut(mut_ptr, 5);
            slice.copy_from_slice(b"hello");

            file.sync_all().unwrap();

            // 验证
            let mut buf = vec![0u8; 5];
            file.read_at(0, &mut buf).unwrap();
            assert_eq!(&buf, b"hello");
        }
    }
}

/// MmapFile 测试（Safe 版本）
#[cfg(test)]
mod mmap_file_tests {
    use super::*;

    #[test]
    fn test_create_with_allocator() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_create.bin");

        let (file, allocator) = MmapFile::create(&path, 1024).unwrap();
        assert_eq!(file.size(), 1024);
        assert_eq!(allocator.total_size(), 1024);
        assert_eq!(allocator.remaining(), 1024);
    }

    #[test]
    fn test_open_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_open.bin");

        // 先创建
        let (file1, mut allocator1) = MmapFile::create(&path, 100).unwrap();
        let range = allocator1.allocate(10).unwrap();
        file1.write_range(range, b"testdata!!").unwrap();
        unsafe { file1.sync_all().unwrap(); }
        drop(file1);
        drop(allocator1);

        // 重新打开
        let (file2, mut allocator2) = MmapFile::open(&path).unwrap();
        assert_eq!(file2.size(), 100);
        assert_eq!(allocator2.total_size(), 100);

        let range = allocator2.allocate(10).unwrap();
        let mut buf = vec![0u8; 10];
        file2.read_range(range, &mut buf).unwrap();
        assert_eq!(&buf, b"testdata!!");
    }

    #[test]
    fn test_basic_write_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_basic.bin");

        let (file, mut allocator) = MmapFile::create(&path, 100).unwrap();

        // 分配并写入
        let range1 = allocator.allocate(10).unwrap();
        let range2 = allocator.allocate(20).unwrap();

        let receipt1 = file.write_range(range1, b"hello_test").unwrap();
        let receipt2 = file.write_range(range2, b"world_test_data_here").unwrap();

        assert_eq!(receipt1.len(), 10);
        assert_eq!(receipt2.len(), 20);

        unsafe { file.sync_all().unwrap(); }

        // 验证读取
        let mut buf1 = vec![0u8; 10];
        let mut buf2 = vec![0u8; 20];

        file.read_range(range1, &mut buf1).unwrap();
        file.read_range(range2, &mut buf2).unwrap();

        assert_eq!(&buf1, b"hello_test");
        assert_eq!(&buf2, b"world_test_data_here");
    }

    #[test]
    fn test_write_range_all() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_write_all.bin");

        let (file, mut allocator) = MmapFile::create(&path, 100).unwrap();

        let range = allocator.allocate(10).unwrap();
        let receipt = file.write_range_all(range, b"1234567890").unwrap();

        assert_eq!(receipt.len(), 10);
        assert_eq!(receipt.start(), 0);
        assert_eq!(receipt.end(), 10);
    }

    #[test]
    fn test_concurrent_writes_with_allocated_ranges() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_concurrent.bin");

        let (file, mut allocator) = MmapFile::create(&path, 1000).unwrap();

        // 预先分配所有范围
        let mut ranges = Vec::new();
        for _ in 0..10 {
            ranges.push(allocator.allocate(100).unwrap());
        }

        // 并发写入（编译期安全！）
        std::thread::scope(|s| {
            for (i, range) in ranges.into_iter().enumerate() {
                let f = file.clone();
                s.spawn(move || {
                    let data = vec![i as u8; 100];
                    let _receipt = f.write_range(range, &data).unwrap();
                });
            }
        });

        unsafe { file.sync_all().unwrap(); }

        // 验证
        let mut allocator2 = RangeAllocator::new(1000);
        for i in 0..10 {
            let range = allocator2.allocate(100).unwrap();
            let mut buf = vec![0u8; 100];
            file.read_range(range, &mut buf).unwrap();
            assert_eq!(buf, vec![i as u8; 100]);
        }
    }

    #[test]
    fn test_high_concurrency() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_high_concurrency.bin");

        // 100个并发写入
        let num_threads = 100;
        let chunk_size = 1024;
        let file_size = num_threads * chunk_size;

        let (file, mut allocator) = MmapFile::create(&path, file_size as u64).unwrap();

        // 预先分配所有范围
        let mut ranges = Vec::new();
        for _ in 0..num_threads {
            ranges.push(allocator.allocate(chunk_size as u64).unwrap());
        }

        // 并发写入
        std::thread::scope(|s| {
            for (i, range) in ranges.into_iter().enumerate() {
                let f = file.clone();
                s.spawn(move || {
                    let data = vec![i as u8; chunk_size];
                    let _receipt = f.write_range(range, &data).unwrap();
                });
            }
        });

        unsafe { file.sync_all().unwrap(); }

        // 验证
        let mut allocator2 = RangeAllocator::new(file_size as u64);
        for i in 0..num_threads {
            let range = allocator2.allocate(chunk_size as u64).unwrap();
            let mut buf = vec![0u8; chunk_size];
            file.read_range(range, &mut buf).unwrap();
            assert_eq!(buf, vec![i as u8; chunk_size]);
        }
    }

    #[test]
    fn test_data_length_mismatch_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_length_mismatch.bin");

        let (file, mut allocator) = MmapFile::create(&path, 100).unwrap();

        let range = allocator.allocate(10).unwrap();

        // 数据长度不匹配 - 太短
        let result = file.write_range(range, b"hello");
        assert!(result.is_err());

        // 数据长度不匹配 - 太长
        let result = file.write_range(range, b"hello world");
        assert!(result.is_err());

        // 正确的长度
        let result = file.write_range(range, b"hello12345");
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_range_buffer_too_small() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_read_buf.bin");

        let (file, mut allocator) = MmapFile::create(&path, 100).unwrap();

        let range = allocator.allocate(10).unwrap();
        file.write_range(range, b"0123456789").unwrap();

        // buffer 太小
        let mut buf = vec![0u8; 5];
        let result = file.read_range(range, &mut buf);
        assert!(result.is_err());

        // buffer 大小正确
        let mut buf = vec![0u8; 10];
        let result = file.read_range(range, &mut buf);
        assert!(result.is_ok());
        assert_eq!(&buf, b"0123456789");

        // buffer 更大也可以
        let mut buf = vec![0u8; 20];
        let result = file.read_range(range, &mut buf);
        assert!(result.is_ok());
        assert_eq!(&buf[..10], b"0123456789");
    }

    #[test]
    fn test_flush_operations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_flush.bin");

        let (file, mut allocator) = MmapFile::create(&path, 1000).unwrap();

        let range1 = allocator.allocate(100).unwrap();
        let range2 = allocator.allocate(100).unwrap();

        let receipt1 = file.write_range(range1, &[1u8; 100]).unwrap();
        let receipt2 = file.write_range(range2, &[2u8; 100]).unwrap();

        // 测试异步刷新
        file.flush().unwrap();

        // 刷新特定范围
        file.flush_range(receipt1).unwrap();
        file.flush_range(receipt2).unwrap();

        // 全局同步
        unsafe { file.sync_all().unwrap(); }
    }

    #[test]
    fn test_write_receipt_properties() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_receipt.bin");

        let (file, mut allocator) = MmapFile::create(&path, 100).unwrap();

        let range = allocator.allocate(50).unwrap();
        let receipt = file.write_range(range, &[0u8; 50]).unwrap();

        assert_eq!(receipt.start(), 0);
        assert_eq!(receipt.end(), 50);
        assert_eq!(receipt.len(), 50);
        assert!(!receipt.is_empty());
        assert_eq!(receipt.range(), range);
    }

    #[test]
    fn test_zero_size_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_zero_size.bin");

        let result = MmapFile::create(&path, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_clone_and_shared_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("safe_clone.bin");

        let (file1, mut allocator) = MmapFile::create(&path, 100).unwrap();
        let file2 = file1.clone();

        let range1 = allocator.allocate(20).unwrap();
        let range2 = allocator.allocate(20).unwrap();

        // 两个克隆分别写入
        file1.write_range(range1, b"from_file1__________").unwrap();
        file2.write_range(range2, b"from_file2__________").unwrap();

        unsafe { file1.sync_all().unwrap(); }

        // 从任一引用读取
        let mut buf1 = vec![0u8; 20];
        let mut buf2 = vec![0u8; 20];
        file2.read_range(range1, &mut buf1).unwrap();
        file1.read_range(range2, &mut buf2).unwrap();

        assert_eq!(&buf1, b"from_file1__________");
        assert_eq!(&buf2, b"from_file2__________");
    }
}

/// RangeAllocator 测试
#[cfg(test)]
mod range_allocator_tests {
    use super::*;

    #[test]
    fn test_new_allocator() {
        let allocator = RangeAllocator::new(1000);
        assert_eq!(allocator.total_size(), 1000);
        assert_eq!(allocator.remaining(), 1000);
        assert_eq!(allocator.next_pos(), 0);
    }

    #[test]
    fn test_sequential_allocation() {
        let mut allocator = RangeAllocator::new(1000);

        let range1 = allocator.allocate(100).unwrap();
        assert_eq!(range1.start(), 0);
        assert_eq!(range1.end(), 100);
        assert_eq!(range1.len(), 100);
        assert_eq!(allocator.next_pos(), 100);
        assert_eq!(allocator.remaining(), 900);

        let range2 = allocator.allocate(200).unwrap();
        assert_eq!(range2.start(), 100);
        assert_eq!(range2.end(), 300);
        assert_eq!(range2.len(), 200);
        assert_eq!(allocator.next_pos(), 300);
        assert_eq!(allocator.remaining(), 700);

        let range3 = allocator.allocate(700).unwrap();
        assert_eq!(range3.start(), 300);
        assert_eq!(range3.end(), 1000);
        assert_eq!(allocator.next_pos(), 1000);
        assert_eq!(allocator.remaining(), 0);
    }

    #[test]
    fn test_allocation_exhaustion() {
        let mut allocator = RangeAllocator::new(100);

        let range1 = allocator.allocate(50).unwrap();
        assert_eq!(range1.len(), 50);
        assert_eq!(allocator.remaining(), 50);

        let range2 = allocator.allocate(50).unwrap();
        assert_eq!(range2.len(), 50);
        assert_eq!(allocator.remaining(), 0);

        // 空间耗尽
        let result = allocator.allocate(1);
        assert!(result.is_none());

        // 继续尝试分配
        let result = allocator.allocate(10);
        assert!(result.is_none());
    }

    #[test]
    fn test_allocate_zero_size() {
        let mut allocator = RangeAllocator::new(100);

        let range = allocator.allocate(0).unwrap();
        assert_eq!(range.start(), 0);
        assert_eq!(range.end(), 0);
        assert_eq!(range.len(), 0);
        assert!(range.is_empty());
        assert_eq!(allocator.remaining(), 100);
    }

    #[test]
    fn test_allocate_exact_remaining() {
        let mut allocator = RangeAllocator::new(100);

        let _range1 = allocator.allocate(30).unwrap();
        assert_eq!(allocator.remaining(), 70);

        // 分配剩余的精确大小
        let range2 = allocator.allocate(70).unwrap();
        assert_eq!(range2.len(), 70);
        assert_eq!(allocator.remaining(), 0);
    }

    #[test]
    fn test_allocate_more_than_total() {
        let mut allocator = RangeAllocator::new(100);

        let result = allocator.allocate(200);
        assert!(result.is_none());
        assert_eq!(allocator.remaining(), 100);
    }

    #[test]
    fn test_multiple_small_allocations() {
        let mut allocator = RangeAllocator::new(100);

        for i in 0..10 {
            let range = allocator.allocate(10).unwrap();
            assert_eq!(range.start(), i * 10);
            assert_eq!(range.end(), (i + 1) * 10);
        }

        assert_eq!(allocator.remaining(), 0);
    }
}

/// AllocatedRange 和 WriteReceipt 测试
#[cfg(test)]
mod types_tests {
    use super::*;
    use std::ops::Range;

    #[test]
    fn test_allocated_range_properties() {
        let mut allocator = RangeAllocator::new(1000);
        let range = allocator.allocate(100).unwrap();

        assert_eq!(range.start(), 0);
        assert_eq!(range.end(), 100);
        assert_eq!(range.len(), 100);
        assert!(!range.is_empty());
    }

    #[test]
    fn test_allocated_range_empty() {
        let mut allocator = RangeAllocator::new(1000);
        let range = allocator.allocate(0).unwrap();

        assert_eq!(range.start(), 0);
        assert_eq!(range.end(), 0);
        assert_eq!(range.len(), 0);
        assert!(range.is_empty());
    }

    #[test]
    fn test_allocated_range_conversions() {
        let mut allocator = RangeAllocator::new(1000);
        let range = allocator.allocate(100).unwrap();

        // 测试 as_range_tuple
        let (start, end) = range.as_range_tuple();
        assert_eq!(start, 0);
        assert_eq!(end, 100);

        // 测试 as_range
        let std_range = range.as_range();
        assert_eq!(std_range, 0..100);

        // 测试 Into<Range<u64>>
        let std_range: Range<u64> = range.into();
        assert_eq!(std_range, 0..100);
    }

    #[test]
    fn test_allocated_range_equality() {
        let mut allocator = RangeAllocator::new(1000);
        let range1 = allocator.allocate(100).unwrap();
        let range2 = allocator.allocate(100).unwrap();
        let range3 = allocator.allocate(100).unwrap();

        // 测试相等性
        assert_eq!(range1, range1);
        assert_ne!(range1, range2);
        assert_ne!(range2, range3);

        // 克隆后相等
        let range1_clone = range1;
        assert_eq!(range1, range1_clone);
    }

    #[test]
    fn test_write_receipt_properties() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("receipt_props.bin");

        let (file, mut allocator) = MmapFile::create(&path, 1000).unwrap();

        let range = allocator.allocate(150).unwrap();
        let receipt = file.write_range(range, &[0u8; 150]).unwrap();

        assert_eq!(receipt.start(), 0);
        assert_eq!(receipt.end(), 150);
        assert_eq!(receipt.len(), 150);
        assert!(!receipt.is_empty());
        assert_eq!(receipt.range(), range);
    }

    #[test]
    fn test_write_receipt_empty_range() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("receipt_empty.bin");

        let (file, mut allocator) = MmapFile::create(&path, 1000).unwrap();

        let range = allocator.allocate(0).unwrap();
        let receipt = file.write_range(range, &[]).unwrap();

        assert_eq!(receipt.start(), 0);
        assert_eq!(receipt.end(), 0);
        assert_eq!(receipt.len(), 0);
        assert!(receipt.is_empty());
    }

    #[test]
    fn test_write_receipt_equality() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("receipt_eq.bin");

        let (file, mut allocator) = MmapFile::create(&path, 1000).unwrap();

        let range1 = allocator.allocate(100).unwrap();
        let range2 = allocator.allocate(100).unwrap();

        let receipt1 = file.write_range(range1, &[1u8; 100]).unwrap();
        let receipt2 = file.write_range(range2, &[2u8; 100]).unwrap();

        assert_eq!(receipt1, receipt1);
        assert_ne!(receipt1, receipt2);

        let receipt1_clone = receipt1;
        assert_eq!(receipt1, receipt1_clone);
    }

    #[test]
    fn test_multiple_receipts_from_same_range() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("receipt_multi.bin");

        let (file, mut allocator) = MmapFile::create(&path, 1000).unwrap();

        let range = allocator.allocate(100).unwrap();

        // 可以多次写入同一个范围（虽然不常见）
        let receipt1 = file.write_range(range, &[1u8; 100]).unwrap();
        let receipt2 = file.write_range(range, &[2u8; 100]).unwrap();

        // 两个凭据应该相等（因为范围相同）
        assert_eq!(receipt1.range(), receipt2.range());
    }
}

