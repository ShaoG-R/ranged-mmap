# ranged-mmap

[![Crates.io](https://img.shields.io/crates/v/ranged-mmap.svg)](https://crates.io/crates/ranged-mmap)
[![Documentation](https://docs.rs/ranged-mmap/badge.svg)](https://docs.rs/ranged-mmap)
[![License](https://img.shields.io/crates/l/ranged-mmap.svg)](https://github.com/ShaoG-R/ranged-mmap#license)

[English](README.md) | [中文](README_CN.md)

一个类型安全的高性能内存映射文件库，专为**无锁并发写入**非重叠范围而优化。

## 特性

- 🚀 **零拷贝写入**：数据直接写入映射内存，无需系统调用
- 🔒 **无锁并发**：多个线程可以同时写入不同的文件区域，无需加锁
- ✅ **类型安全 API**：通过类型系统在编译期防止重叠写入
- 📦 **引用计数**：可以克隆并在多个 worker 间共享
- ⚡ **高性能**：专为并发随机写入优化（参见性能测试）
- 🔧 **手动刷盘**：精细控制何时将数据同步到磁盘
- 🌐 **运行时无关**：可与任何异步运行时（tokio、async-std）配合使用，或不使用运行时

## 使用场景

**适用于：**
- 🌐 **多线程下载器**：并发写入不同的文件块
- 📝 **日志系统**：多个线程写入不同的日志区域
- 💾 **数据库系统**：并发更新不同的数据块
- 📊 **大文件处理**：并行处理文件的不同部分

**不适用于：**
- 需要动态调整大小的文件（创建时必须指定大小）
- 顺序或小文件操作（开销不值得）
- 虚拟内存有限的系统

## 快速开始

添加到 `Cargo.toml`：

```toml
[dependencies]
ranged-mmap = "0.1"
```

### 类型安全版本（推荐）

`MmapFile` API 通过范围分配提供编译期安全保证：

```rust
use ranged_mmap::MmapFile;

fn main() -> std::io::Result<()> {
    // 创建 1MB 文件和范围分配器
    let (file, mut allocator) = MmapFile::create("output.bin", 1024 * 1024)?;

    // 在主线程分配不重叠的范围
    let range1 = allocator.allocate(512 * 1024).unwrap(); // [0, 512KB)
    let range2 = allocator.allocate(512 * 1024).unwrap(); // [512KB, 1MB)

    // 并发写入不同范围（编译期安全！）
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

    // 最后同步刷新确保所有数据写入磁盘
    unsafe { file.sync_all()?; }
    Ok(())
}
```

### Unsafe 版本（最大性能）

对于可以手动保证不重叠写入的场景：

```rust
use ranged_mmap::MmapFileInner;

fn main() -> std::io::Result<()> {
    let file = MmapFileInner::create("output.bin", 1024)?;

    let file1 = file.clone();
    let file2 = file.clone();

    std::thread::scope(|s| {
        // ⚠️ 安全性：你必须确保区域不重叠
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

## API 概览

### 主要类型

- **`MmapFile`**：类型安全的内存映射文件，提供编译期安全保证
- **`MmapFileInner`**：Unsafe 高性能版本，需要手动管理安全性
- **`RangeAllocator`**：顺序分配不重叠的文件范围
- **`AllocatedRange`**：表示有效且不重叠的文件范围
- **`WriteReceipt`**：证明范围已被写入的凭据（实现类型安全的刷新）

### 核心方法

#### `MmapFile`（类型安全）

```rust
// 创建文件和分配器
let (file, mut allocator) = MmapFile::create(path, size)?;

// 分配范围（主线程）
let range = allocator.allocate(1024)?;

// 写入范围（返回凭据）
let receipt = file.write_range(range, data)?;

// 使用凭据刷新
file.flush_range(receipt)?;

// 同步所有数据到磁盘
unsafe { file.sync_all()?; }
```

#### `MmapFileInner`（Unsafe）

```rust
// 创建文件
let file = MmapFileInner::create(path, size)?;

// 在指定偏移处写入（必须确保不重叠）
unsafe { file.write_at(offset, data)?; }

// 刷新到磁盘
unsafe { file.flush()?; }
```

## 安全性保证

### 编译期安全（`MmapFile`）

类型系统确保：
- ✅ 所有范围都通过 `RangeAllocator` 分配
- ✅ 范围顺序分配，防止重叠
- ✅ 数据长度必须匹配范围长度
- ✅ 只能刷新已写入的范围（通过 `WriteReceipt`）

### 运行时安全（`MmapFileInner`）

你必须确保：
- ⚠️ 不同线程写入不重叠的内存区域
- ⚠️ 写入期间不会读取同一区域
- ⚠️ 如果违反上述规则，需要适当的同步

## 性能

本库专为并发随机写入场景优化。与标准 `tokio::fs::File` 相比：

- **写入零系统调用**：直接修改内存
- **无需锁**：真正的并行写入不同区域
- **批量刷盘**：控制何时将数据同步到磁盘

详细性能测试请参见 `benches/concurrent_write.rs`。

## 高级用法

### 与 Tokio 运行时配合

```rust
use ranged_mmap::MmapFile;
use tokio::task;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (file, mut allocator) = MmapFile::create("output.bin", 1024 * 1024)?;
    
    // 分配范围
    let range1 = allocator.allocate(512 * 1024).unwrap();
    let range2 = allocator.allocate(512 * 1024).unwrap();
    
    // 派生异步任务
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
    
    // 刷新特定范围
    file.flush_range(receipt1)?;
    file.flush_range(receipt2)?;
    
    unsafe { file.sync_all()?; }
    Ok(())
}
```

### 读取数据

```rust
use ranged_mmap::MmapFile;

fn main() -> std::io::Result<()> {
    let (file, mut allocator) = MmapFile::create("output.bin", 1024)?;
    let range = allocator.allocate(100).unwrap();
    
    // 写入数据
    file.write_range(range, &[42u8; 100])?;
    
    // 读回数据
    let mut buf = vec![0u8; 100];
    file.read_range(range, &mut buf)?;
    
    assert_eq!(buf, vec![42u8; 100]);
    Ok(())
}
```

### 打开已存在的文件

```rust
use ranged_mmap::MmapFile;

fn main() -> std::io::Result<()> {
    // 打开已存在的文件
    let (file, mut allocator) = MmapFile::open("existing.bin")?;
    
    println!("文件大小: {} 字节", file.size());
    println!("剩余可分配: {} 字节", allocator.remaining());
    
    // 继续分配和写入
    if let Some(range) = allocator.allocate(1024) {
        file.write_range(range, &[0u8; 1024])?;
    }
    
    Ok(())
}
```

## 限制

- **固定大小**：创建时必须指定文件大小，不能动态调整
- **虚拟内存**：最大文件大小受系统虚拟内存限制
- **平台支持**：目前针对类 Unix 系统和 Windows 优化
- **无内置锁**：用户必须管理并发访问模式

## 工作原理

1. **内存映射**：使用 `memmap2` 将文件映射为连续的内存区域
2. **范围分配**：`RangeAllocator` 顺序分配不重叠的范围
3. **类型安全**：`AllocatedRange` 只能通过分配器创建，保证有效性
4. **无锁写入**：每个线程写入自己的 `AllocatedRange`，避免加锁
5. **手动刷盘**：用户控制何时将数据同步到磁盘，实现最佳性能

## 对比

| 特性 | ranged-mmap | tokio::fs::File | std::fs::File |
|------|-------------|-----------------|---------------|
| 并发写入 | ✅ 无锁 | ❌ 需要锁 | ❌ 需要锁 |
| 零拷贝 | ✅ 是 | ❌ 否 | ❌ 否 |
| 类型安全 | ✅ 编译期 | ⚠️ 运行时 | ⚠️ 运行时 |
| 系统调用（写入） | ✅ 零次 | ❌ 每次写入 | ❌ 每次写入 |
| 动态大小 | ❌ 固定 | ✅ 是 | ✅ 是 |
| 异步支持 | ✅ 运行时无关 | ✅ 仅 Tokio | ❌ 否 |

## 贡献

欢迎贡献！请随时提交 issue 或 pull request。

## 许可证

本项目采用以下任一许可证：

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)

由你选择。

## 致谢

基于优秀的 [memmap2](https://github.com/RazrFalcon/memmap2-rs) crate 构建。

