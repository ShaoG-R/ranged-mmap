# ranged-mmap

[![Crates.io](https://img.shields.io/crates/v/ranged-mmap.svg)](https://crates.io/crates/ranged-mmap)
[![Documentation](https://docs.rs/ranged-mmap/badge.svg)](https://docs.rs/ranged-mmap)
[![License](https://img.shields.io/crates/l/ranged-mmap.svg)](https://github.com/ShaoG-R/ranged-mmap#license)

[English](README.md) | [中文](README_CN.md)

A type-safe, high-performance memory-mapped file library optimized for **lock-free concurrent writes** to non-overlapping ranges.

## Features

- 🚀 **Zero-Copy Writes**: Data is written directly to mapped memory without system calls
- 🔒 **Lock-Free Concurrency**: Multiple threads can write to different file regions simultaneously without locks
- ✅ **Type-Safe API**: Prevents overlapping writes at compile-time through the type system
- 📦 **Reference Counting**: Can be cloned and shared among multiple workers
- ⚡ **High Performance**: Optimized for concurrent random writes (see benchmarks)
- 🔧 **Manual Flushing**: Fine-grained control over when data is synchronized to disk
- 🌐 **Runtime Agnostic**: Works with any async runtime (tokio, async-std) or without one
- 📐 **4K Alignment**: All allocations are automatically aligned to 4K boundaries for optimal I/O performance
- 🔄 **Dual Allocators**: Sequential allocator for single-thread use, wait-free concurrent allocator for multi-thread scenarios

## When to Use

**Perfect for:**
- 🌐 **Multi-threaded downloaders**: Concurrent writes to different file chunks
- 📝 **Logging systems**: Multiple threads writing to different log regions
- 💾 **Database systems**: Concurrent updates to different data blocks
- 📊 **Large file processing**: Parallel processing of different file sections

**Not suitable for:**
- Files that need dynamic resizing (size must be known at creation)
- Sequential or small file operations (overhead not justified)
- Systems with limited virtual memory

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ranged-mmap = "0.3"
```

### Type-Safe Version (Recommended)

The `MmapFile` API provides compile-time safety guarantees through range allocation:

```rust
use ranged_mmap::{MmapFile, allocator::ALIGNMENT};
use std::num::NonZeroU64;

fn main() -> ranged_mmap::Result<()> {
    // Create a file (size in 4K units) and range allocator
    // All allocations are 4K aligned automatically
    let (file, mut allocator) = MmapFile::create_default(
        "output.bin",
        NonZeroU64::new(ALIGNMENT * 256).unwrap()  // 1MB (256 * 4K)
    )?;

    // Allocate non-overlapping ranges in the main thread (4K aligned)
    let range1 = allocator.allocate(NonZeroU64::new(ALIGNMENT * 128).unwrap()).unwrap(); // [0, 512KB)
    let range2 = allocator.allocate(NonZeroU64::new(ALIGNMENT * 128).unwrap()).unwrap(); // [512KB, 1MB)

    // Concurrent writes to different ranges (compile-time safe!)
    std::thread::scope(|s| {
        let f1 = file.clone();
        let f2 = file.clone();
        
        s.spawn(move || {
            let receipt = f1.write_range(range1, &vec![1u8; (ALIGNMENT * 128) as usize]);
            f1.flush_range(receipt);
        });
        
        s.spawn(move || {
            let receipt = f2.write_range(range2, &vec![2u8; (ALIGNMENT * 128) as usize]);
            f2.flush_range(receipt);
        });
    });

    // Final synchronous flush to ensure all data is written
    unsafe { file.sync_all()?; }
    Ok(())
}
```

### Unsafe Version (Maximum Performance)

For scenarios where you can manually guarantee non-overlapping writes:

```rust
use ranged_mmap::MmapFileInner;
use std::num::NonZeroU64;

fn main() -> ranged_mmap::Result<()> {
    let file = MmapFileInner::create("output.bin", NonZeroU64::new(1024).unwrap())?;

    let file1 = file.clone();
    let file2 = file.clone();

    std::thread::scope(|s| {
        // ⚠️ Safety: You must ensure non-overlapping regions
        s.spawn(|| unsafe { 
            file1.write_at(0, &[1; 512]);
        });
        s.spawn(|| unsafe { 
            file2.write_at(512, &[2; 512]);
        });
    });

    unsafe { file.flush()?; }
    Ok(())
}
```

## API Overview

### Main Types

- **`MmapFile`**: Type-safe memory-mapped file with compile-time safety
- **`MmapFileInner`**: Unsafe high-performance version for manual safety management
- **`RangeAllocator`**: Trait for range allocators
- **`allocator::sequential::Allocator`**: Sequential allocator for single-thread use
- **`allocator::concurrent::Allocator`**: Wait-free concurrent allocator for multi-thread scenarios
- **`AllocatedRange`**: Represents a valid, non-overlapping file range
- **`WriteReceipt`**: Proof that a range has been written (enables type-safe flushing)
- **`SplitResult`**: Result of splitting an allocated range at a 4K-aligned position
- **`ALIGNMENT`**: 4K alignment constant (4096 bytes)
- **`align_up`**: Function to align values up to 4K boundary
- **`align_down`**: Function to align values down to 4K boundary

### Core Methods

#### `MmapFile` (Type-Safe)

```rust
use std::num::NonZeroU64;
use ranged_mmap::allocator::{sequential, concurrent, ALIGNMENT};

// Create file with default sequential allocator
let (file, mut allocator) = MmapFile::create_default(path, NonZeroU64::new(size).unwrap())?;

// Or specify allocator type explicitly
let (file, mut allocator) = MmapFile::create::<sequential::Allocator>(path, NonZeroU64::new(size).unwrap())?;

// Use concurrent allocator for multi-thread allocation
let (file, allocator) = MmapFile::create::<concurrent::Allocator>(path, NonZeroU64::new(size).unwrap())?;

// Allocate ranges (4K aligned, returns Option)
let range = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()).unwrap();

// Write to range (returns receipt directly)
let receipt = file.write_range(range, data);

// Flush using receipt
file.flush_range(receipt);

// Sync all data to disk
unsafe { file.sync_all()?; }
```

#### `MmapFileInner` (Unsafe)

```rust
use std::num::NonZeroU64;

// Create file
let file = MmapFileInner::create(path, NonZeroU64::new(size).unwrap())?;

// Write at offset (must ensure non-overlapping)
unsafe { file.write_at(offset, data); }

// Flush to disk
unsafe { file.flush()?; }
```

## Safety Guarantees

### Compile-Time Safety (`MmapFile`)

The type system ensures:
- ✅ All ranges are allocated through `RangeAllocator`
- ✅ Ranges are allocated sequentially/atomically, preventing overlaps
- ✅ All allocations are 4K aligned for optimal I/O performance
- ✅ Data length must match range length
- ✅ Only written ranges can be flushed (via `WriteReceipt`)

### Runtime Safety (`MmapFileInner`)

You must ensure:
- ⚠️ Different threads write to non-overlapping memory regions
- ⚠️ No reads occur to a region during writes
- ⚠️ Proper synchronization if violating the above rules

## Performance

This library is optimized for concurrent random write scenarios. Compared to standard `tokio::fs::File`, it offers:

- **Zero system calls for writes**: Direct memory modification
- **No locks required**: True parallel writes to different regions
- **Batch flushing**: Control when data is synchronized to disk

See `benches/concurrent_write.rs` for detailed benchmarks.

## Advanced Usage

### With Concurrent Allocator (Multi-Thread Allocation)

```rust
use ranged_mmap::{MmapFile, allocator::{concurrent, ALIGNMENT}};
use std::num::NonZeroU64;
use std::sync::Arc;

fn main() -> ranged_mmap::Result<()> {
    // Use concurrent allocator for wait-free allocation from multiple threads
    let (file, allocator) = MmapFile::create::<concurrent::Allocator>(
        "output.bin",
        NonZeroU64::new(ALIGNMENT * 100).unwrap()
    )?;
    let allocator = Arc::new(allocator);
    
    std::thread::scope(|s| {
        for _ in 0..4 {
            let f = file.clone();
            let alloc = Arc::clone(&allocator);
            s.spawn(move || {
                // Each thread can allocate independently (wait-free)
                while let Some(range) = alloc.allocate(NonZeroU64::new(ALIGNMENT).unwrap()) {
                    let receipt = f.write_range(range, &vec![42u8; ALIGNMENT as usize]);
                    f.flush_range(receipt);
                }
            });
        }
    });
    
    unsafe { file.sync_all()?; }
    Ok(())
}
```

### With Tokio Runtime

```rust
use ranged_mmap::{MmapFile, allocator::ALIGNMENT};
use std::num::NonZeroU64;
use tokio::task;

#[tokio::main]
async fn main() -> ranged_mmap::Result<()> {
    let (file, mut allocator) = MmapFile::create_default(
        "output.bin",
        NonZeroU64::new(ALIGNMENT * 256).unwrap()  // 1MB
    )?;
    
    // Allocate ranges (4K aligned)
    let range1 = allocator.allocate(NonZeroU64::new(ALIGNMENT * 128).unwrap()).unwrap();
    let range2 = allocator.allocate(NonZeroU64::new(ALIGNMENT * 128).unwrap()).unwrap();
    
    // Spawn async tasks
    let f1 = file.clone();
    let f2 = file.clone();
    
    let task1 = task::spawn_blocking(move || {
        f1.write_range(range1, &vec![1u8; (ALIGNMENT * 128) as usize])
    });
    
    let task2 = task::spawn_blocking(move || {
        f2.write_range(range2, &vec![2u8; (ALIGNMENT * 128) as usize])
    });
    
    let receipt1 = task1.await.unwrap();
    let receipt2 = task2.await.unwrap();
    
    // Flush specific ranges
    file.flush_range(receipt1);
    file.flush_range(receipt2);
    
    unsafe { file.sync_all()?; }
    Ok(())
}
```

### Reading Data

```rust
use ranged_mmap::{MmapFile, allocator::ALIGNMENT};
use std::num::NonZeroU64;

fn main() -> ranged_mmap::Result<()> {
    let (file, mut allocator) = MmapFile::create_default(
        "output.bin",
        NonZeroU64::new(ALIGNMENT).unwrap()
    )?;
    // Allocations are 4K aligned
    let range = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()).unwrap();
    
    // Write data (data length must match range length)
    file.write_range(range, &vec![42u8; ALIGNMENT as usize]);
    
    // Read back
    let mut buf = vec![0u8; ALIGNMENT as usize];
    file.read_range(range, &mut buf)?;
    
    assert_eq!(buf[0], 42u8);
    Ok(())
}
```

### Opening Existing Files

```rust
use ranged_mmap::{MmapFile, allocator::ALIGNMENT};
use std::num::NonZeroU64;

fn main() -> ranged_mmap::Result<()> {
    // Open existing file with default sequential allocator
    let (file, mut allocator) = MmapFile::open_default("existing.bin")?;
    
    println!("File size: {} bytes", file.size());
    println!("Remaining allocatable: {} bytes", allocator.remaining());
    
    // Continue allocating and writing (4K aligned)
    if let Some(range) = allocator.allocate(NonZeroU64::new(ALIGNMENT).unwrap()) {
        file.write_range(range, &vec![0u8; ALIGNMENT as usize]);
    }
    
    Ok(())
}
```

## Limitations

- **Fixed Size**: File size must be specified at creation and cannot be changed
- **Virtual Memory**: Maximum file size is limited by system virtual memory
- **Platform Support**: Currently optimized for Unix-like systems and Windows
- **No Built-in Locking**: Users must manage concurrent access patterns

## How It Works

1. **Memory Mapping**: The file is memory-mapped using `memmap2`, making it accessible as a continuous memory region
2. **Range Allocation**: Allocators provide non-overlapping ranges:
   - `sequential::Allocator`: Simple sequential allocation for single-thread use
   - `concurrent::Allocator`: Wait-free atomic allocation for multi-thread scenarios
3. **4K Alignment**: All allocations are aligned to 4K boundaries for optimal I/O performance
4. **Type Safety**: `AllocatedRange` can only be created through the allocator, guaranteeing validity
5. **Lock-Free Writes**: Each thread writes to its own `AllocatedRange`, avoiding locks
6. **Manual Flushing**: Users control when data is synchronized to disk for optimal performance

## Comparison

| Feature | ranged-mmap | tokio::fs::File | std::fs::File |
|---------|-------------|-----------------|---------------|
| Concurrent writes | ✅ Lock-free | ❌ Requires locks | ❌ Requires locks |
| Zero-copy | ✅ Yes | ❌ No | ❌ No |
| Type safety | ✅ Compile-time | ⚠️ Runtime | ⚠️ Runtime |
| System calls (write) | ✅ Zero | ❌ Per write | ❌ Per write |
| Dynamic size | ❌ Fixed | ✅ Yes | ✅ Yes |
| Async support | ✅ Runtime agnostic | ✅ Tokio only | ❌ No |

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

Built on top of the excellent [memmap2](https://github.com/RazrFalcon/memmap2-rs) crate.

