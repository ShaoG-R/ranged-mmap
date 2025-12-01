use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ranged_mmap::MmapFile;
use tempfile::tempdir;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use std::io::SeekFrom;
use std::num::NonZeroU64;

/// 测试参数
const FILE_SIZE: u64 = 1024 * 1024 * 512; // 512MB
const CHUNK_SIZE: usize = 12 * 1024 * 1024; // 12MB
const NUM_WORKERS: usize = 12; // 12个并发协程/线程

/// 使用 tokio::fs::File 进行分段并发写入
async fn bench_tokio_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("tokio_test.bin");

    // 创建文件并预分配大小
    let file = tokio::fs::File::create(&path).await.unwrap();
    file.set_len(FILE_SIZE).await.unwrap();
    drop(file);

    // 计算总共有多少个chunk
    let total_chunks = (FILE_SIZE as usize + CHUNK_SIZE - 1) / CHUNK_SIZE;
    
    // 创建chunk索引列表
    let chunks: Vec<usize> = (0..total_chunks).collect();
    
    // 使用 NUM_WORKERS 个并发任务写入
    let mut handles = vec![];
    
    for worker_id in 0..NUM_WORKERS {
        let chunks_clone = chunks.clone();
        let path_clone = path.clone();
        
        let handle = tokio::spawn(async move {
            // 每个worker打开自己的文件句柄
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&path_clone)
                .await
                .unwrap();
            
            // 处理分配给这个worker的chunks
            for (idx, &chunk_idx) in chunks_clone.iter().enumerate() {
                // 使用轮询方式分配任务给不同的worker
                if idx % NUM_WORKERS != worker_id {
                    continue;
                }
                
                let offset = (chunk_idx * CHUNK_SIZE) as u64;
                let size = if offset + CHUNK_SIZE as u64 > FILE_SIZE {
                    (FILE_SIZE - offset) as usize
                } else {
                    CHUNK_SIZE
                };
                
                let data = vec![chunk_idx as u8; size];
                
                // tokio::fs::File 需要手动 seek + write
                file.seek(SeekFrom::Start(offset)).await.unwrap();
                file.write_all(&data).await.unwrap();
            }
            
            file.sync_all().await.unwrap();
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
}

/// 使用 MmapFile + tokio 进行分段并发写入
async fn bench_mmap_file_tokio() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mmap_tokio_test.bin");

    // 创建文件并获取分配器
    let (file, mut allocator) = MmapFile::create_default(&path, NonZeroU64::new(FILE_SIZE).unwrap()).unwrap();

    // 计算总共有多少个chunk
    let total_chunks = (FILE_SIZE as usize + CHUNK_SIZE - 1) / CHUNK_SIZE;
    
    // 在主线程预先分配所有范围（保证不重叠）
    let mut ranges = Vec::new();
    for chunk_idx in 0..total_chunks {
        let offset = (chunk_idx * CHUNK_SIZE) as u64;
        let size = if offset + CHUNK_SIZE as u64 > FILE_SIZE {
            (FILE_SIZE - offset) as u64
        } else {
            CHUNK_SIZE as u64
        };
        
        ranges.push((chunk_idx, allocator.allocate(NonZeroU64::new(size).unwrap()).unwrap()));
    }
    
    // 使用 NUM_WORKERS 个并发任务写入
    let mut handles = vec![];
    
    for worker_id in 0..NUM_WORKERS {
        let ranges_clone = ranges.clone();
        let file_clone = file.clone();
        
        let handle = tokio::spawn(async move {
            // 处理分配给这个worker的chunks
            for (idx, &(chunk_idx, range)) in ranges_clone.iter().enumerate() {
                // 使用轮询方式分配任务给不同的worker
                if idx % NUM_WORKERS != worker_id {
                    continue;
                }
                
                let size = range.len() as usize;
                let data = vec![chunk_idx as u8; size];
                
                // MmapFile 使用 write_range 进行安全的并发写入
                file_clone.write_range(range, &data);
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    unsafe { file.sync_all().unwrap(); }
}

/// 使用 MmapFile + std::thread 进行分段并发写入
fn bench_mmap_file_threads() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mmap_threads_test.bin");

    // 创建文件并获取分配器
    let (file, mut allocator) = MmapFile::create_default(&path, NonZeroU64::new(FILE_SIZE).unwrap()).unwrap();

    // 计算总共有多少个chunk
    let total_chunks = (FILE_SIZE as usize + CHUNK_SIZE - 1) / CHUNK_SIZE;
    
    // 在主线程预先分配所有范围（保证不重叠）
    let mut ranges = Vec::new();
    for chunk_idx in 0..total_chunks {
        let offset = (chunk_idx * CHUNK_SIZE) as u64;
        let size = if offset + CHUNK_SIZE as u64 > FILE_SIZE {
            (FILE_SIZE - offset) as u64
        } else {
            CHUNK_SIZE as u64
        };
        
        ranges.push((chunk_idx, allocator.allocate(NonZeroU64::new(size).unwrap()).unwrap()));
    }
    
    // 使用 NUM_WORKERS 个线程写入
    std::thread::scope(|s| {
        for worker_id in 0..NUM_WORKERS {
            let ranges_clone = ranges.clone();
            let file_clone = file.clone();
            
            s.spawn(move || {
                // 处理分配给这个worker的chunks
                for (idx, &(chunk_idx, range)) in ranges_clone.iter().enumerate() {
                    // 使用轮询方式分配任务给不同的worker
                    if idx % NUM_WORKERS != worker_id {
                        continue;
                    }
                    
                    let size = range.len() as usize;
                    let data = vec![chunk_idx as u8; size];
                    
                    // MmapFile 使用 write_range 进行安全的并发写入
                    file_clone.write_range(range, &data);
                }
            });
        }
    });
    
    unsafe { file.sync_all().unwrap(); }
}

fn concurrent_write_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_write");
    
    // 设置更长的测试时间
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));
    
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    group.bench_function(
        BenchmarkId::new(
            "tokio_file", 
            format!("{}MB_{}MB_chunks_{}workers", FILE_SIZE / (1024*1024), CHUNK_SIZE / (1024*1024), NUM_WORKERS)
        ),
        |b| {
            b.to_async(&runtime).iter(|| async {
                bench_tokio_file().await;
            });
        },
    );
    
    group.bench_function(
        BenchmarkId::new(
            "mmap_file_tokio",
            format!("{}MB_{}MB_chunks_{}workers", FILE_SIZE / (1024*1024), CHUNK_SIZE / (1024*1024), NUM_WORKERS)
        ),
        |b| {
            b.to_async(&runtime).iter(|| async {
                bench_mmap_file_tokio().await;
            });
        },
    );
    
    group.bench_function(
        BenchmarkId::new(
            "mmap_file_threads",
            format!("{}MB_{}MB_chunks_{}workers", FILE_SIZE / (1024*1024), CHUNK_SIZE / (1024*1024), NUM_WORKERS)
        ),
        |b| {
            b.iter(|| {
                bench_mmap_file_threads();
            });
        },
    );
    
    group.finish();
}

criterion_group!(benches, concurrent_write_benchmark);
criterion_main!(benches);
