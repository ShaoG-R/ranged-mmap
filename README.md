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
ranged-mmap = "0.1"
```

### Type-Safe Version (Recommended)

The `MmapFile` API provides compile-time safety guarantees through range allocation:

```rust
use ranged_mmap::MmapFile;

fn main() -> std::io::Result<()> {
    // Create a 1MB file and range allocator
    let (file, mut allocator) = MmapFile::create("output.bin", 1024 * 1024)?;

    // Allocate non-overlapping ranges in the main thread
    let range1 = allocator.allocate(512 * 1024).unwrap(); // [0, 512KB)
    let range2 = allocator.allocate(512 * 1024).unwrap(); // [512KB, 1MB)

    // Concurrent writes to different ranges (compile-time safe!)
    std::thread::scope(|s| {
        let f1 = file.clone();
        let f2 = file.clone();
        
        s.spawn(move || {
            let receipt = f1.write_range(range1, &vec![1u8; 512 * 1024]).unwrap();
            f1.flush_range(receipt).unwrap();
        });
        
        s.spawn(move || {
            let receipt = f2.write_range(range2, &vec![2u8; 512 * 1024]).unwrap();
            f2.flush_range(receipt).unwrap();
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

fn main() -> std::io::Result<()> {
    let file = MmapFileInner::create("output.bin", 1024)?;

    let file1 = file.clone();
    let file2 = file.clone();

    std::thread::scope(|s| {
        // ⚠️ Safety: You must ensure non-overlapping regions
        s.spawn(|| unsafe { 
            file1.write_at(0, &[1; 512]).unwrap();
        });
        s.spawn(|| unsafe { 
            file2.write_at(512, &[2; 512]).unwrap();
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
- **`RangeAllocator`**: Allocates non-overlapping file ranges sequentially
- **`AllocatedRange`**: Represents a valid, non-overlapping file range
- **`WriteReceipt`**: Proof that a range has been written (enables type-safe flushing)

### Core Methods

#### `MmapFile` (Type-Safe)

```rust
// Create file and allocator
let (file, mut allocator) = MmapFile::create(path, size)?;

// Allocate ranges (main thread)
let range = allocator.allocate(1024)?;

// Write to range (returns receipt)
let receipt = file.write_range(range, data)?;

// Flush using receipt
file.flush_range(receipt)?;

// Sync all data to disk
unsafe { file.sync_all()?; }
```

#### `MmapFileInner` (Unsafe)

```rust
// Create file
let file = MmapFileInner::create(path, size)?;

// Write at offset (must ensure non-overlapping)
unsafe { file.write_at(offset, data)?; }

// Flush to disk
unsafe { file.flush()?; }
```

## Safety Guarantees

### Compile-Time Safety (`MmapFile`)

The type system ensures:
- ✅ All ranges are allocated through `RangeAllocator`
- ✅ Ranges are allocated sequentially, preventing overlaps
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

### With Tokio Runtime

```rust
use ranged_mmap::MmapFile;
use tokio::task;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (file, mut allocator) = MmapFile::create("output.bin", 1024 * 1024)?;
    
    // Allocate ranges
    let range1 = allocator.allocate(512 * 1024).unwrap();
    let range2 = allocator.allocate(512 * 1024).unwrap();
    
    // Spawn async tasks
    let f1 = file.clone();
    let f2 = file.clone();
    
    let task1 = task::spawn_blocking(move || {
        f1.write_range(range1, &vec![1u8; 512 * 1024])
    });
    
    let task2 = task::spawn_blocking(move || {
        f2.write_range(range2, &vec![2u8; 512 * 1024])
    });
    
    let receipt1 = task1.await.unwrap()?;
    let receipt2 = task2.await.unwrap()?;
    
    // Flush specific ranges
    file.flush_range(receipt1)?;
    file.flush_range(receipt2)?;
    
    unsafe { file.sync_all()?; }
    Ok(())
}
```

### Reading Data

```rust
use ranged_mmap::MmapFile;

fn main() -> std::io::Result<()> {
    let (file, mut allocator) = MmapFile::create("output.bin", 1024)?;
    let range = allocator.allocate(100).unwrap();
    
    // Write data
    file.write_range(range, &[42u8; 100])?;
    
    // Read back
    let mut buf = vec![0u8; 100];
    file.read_range(range, &mut buf)?;
    
    assert_eq!(buf, vec![42u8; 100]);
    Ok(())
}
```

### Opening Existing Files

```rust
use ranged_mmap::MmapFile;

fn main() -> std::io::Result<()> {
    // Open existing file
    let (file, mut allocator) = MmapFile::open("existing.bin")?;
    
    println!("File size: {} bytes", file.size());
    println!("Remaining allocatable: {} bytes", allocator.remaining());
    
    // Continue allocating and writing
    if let Some(range) = allocator.allocate(1024) {
        file.write_range(range, &[0u8; 1024])?;
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
2. **Range Allocation**: `RangeAllocator` sequentially allocates non-overlapping ranges
3. **Type Safety**: `AllocatedRange` can only be created through the allocator, guaranteeing validity
4. **Lock-Free Writes**: Each thread writes to its own `AllocatedRange`, avoiding locks
5. **Manual Flushing**: Users control when data is synchronized to disk for optimal performance

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

