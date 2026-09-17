//! 基准测试：CPU 吞吐、内存带宽（STREAM copy/scale）、磁盘顺序读写。
//!
//! 这些是用户态微基准，不是 SPEC/iozone。结果用于相对对比，
//! 磁盘测试默认写在临时目录，避免误伤系统盘分区表。

use std::fs::{self, File, OpenOptions};
use std::hint::black_box;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Clone, Debug)]
pub struct BenchRequest {
    pub cpu: bool,
    pub memory: bool,
    pub disk: bool,
    pub duration: Duration,
    pub mem_bytes: usize,
    pub disk_bytes: usize,
    pub disk_path: Option<PathBuf>,
}

impl Default for BenchRequest {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            disk: true,
            duration: Duration::from_secs(2),
            mem_bytes: 64 * 1024 * 1024,
            disk_bytes: 64 * 1024 * 1024,
            disk_path: None,
        }
    }
}

impl BenchRequest {
    pub fn quick() -> Self {
        Self {
            duration: Duration::from_millis(200),
            mem_bytes: 4 * 1024 * 1024,
            disk_bytes: 4 * 1024 * 1024,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchReport {
    pub cpu: Option<CpuBench>,
    pub memory: Option<MemBench>,
    pub disk: Option<DiskBench>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpuBench {
    pub threads: usize,
    pub duration_ms: u128,
    pub integer_mops: f64,
    pub float_mflops: f64,
    /// 无量纲相对分：整数 Mops + 浮点 MFLOPS，便于自己前后对比。
    pub score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemBench {
    pub bytes: usize,
    pub copy_gbs: f64,
    pub scale_gbs: f64,
    pub triad_gbs: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiskBench {
    pub path: String,
    pub bytes: usize,
    pub write_mbs: f64,
    pub read_mbs: f64,
    pub fsync_ms: u128,
}

pub fn run(req: &BenchRequest) -> BenchReport {
    let mut notes = Vec::new();
    notes.push(
        "微基准受 Turbo、调度器、后台负载和文件系统缓存影响；磁盘顺序读写在 page cache 下会偏高，只能作相对参考。"
            .into(),
    );
    let cpu = req.cpu.then(|| cpu_bench(req.duration));
    let memory = req.memory.then(|| mem_bench(req.mem_bytes));
    let disk = req.disk.then(|| disk_bench(req.disk_bytes, req.disk_path.clone(), &mut notes));
    BenchReport {
        cpu,
        memory,
        disk,
        notes,
    }
}

fn cpu_bench(duration: Duration) -> CpuBench {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            std::thread::spawn(move || {
                let mut int_ops: u64 = 0;
                let mut fp_ops: u64 = 0;
                let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ i as u64;
                let mut y: f64 = 1.0001 + i as f64 * 0.001;
                let t0 = Instant::now();
                while t0.elapsed() < duration {
                    for _ in 0..4096 {
                        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                        x ^= x >> 17;
                        y = y.mul_add(1.000000119, 0.5).sin().mul_add(0.5, 0.5);
                        int_ops += 1;
                        fp_ops += 2;
                    }
                }
                black_box(x);
                black_box(y);
                (int_ops, fp_ops)
            })
        })
        .collect();
    let mut int_ops = 0u64;
    let mut fp_ops = 0u64;
    for h in handles {
        if let Ok((i, f)) = h.join() {
            int_ops += i;
            fp_ops += f;
        }
    }
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64().max(1e-9);
    let integer_mops = (int_ops as f64) / secs / 1_000_000.0;
    let float_mflops = (fp_ops as f64) / secs / 1_000_000.0;
    CpuBench {
        threads,
        duration_ms: elapsed.as_millis(),
        integer_mops,
        float_mflops,
        score: integer_mops + float_mflops,
    }
}

fn mem_bench(bytes: usize) -> MemBench {
    let n = (bytes / std::mem::size_of::<f64>()).max(1024);
    let a = vec![1.0f64; n];
    let mut b = vec![0.0f64; n];
    let mut c = vec![0.0f64; n];
    // 预热，避免第一次缺页主导结果。
    for i in 0..n {
        b[i] = a[i];
    }
    let scalar = 3.0f64;
    let copy = timed_gbs(n * 8 * 2, || {
        for i in 0..n {
            b[i] = a[i];
        }
        black_box(&b);
    });
    let scale = timed_gbs(n * 8 * 2, || {
        for i in 0..n {
            b[i] = scalar * a[i];
        }
        black_box(&b);
    });
    let triad = timed_gbs(n * 8 * 3, || {
        for i in 0..n {
            c[i] = a[i] + scalar * b[i];
        }
        black_box(&c);
    });
    MemBench {
        bytes: n * 8,
        copy_gbs: copy,
        scale_gbs: scale,
        triad_gbs: triad,
    }
}

fn timed_gbs(moved: usize, mut f: impl FnMut()) -> f64 {
    let t = Instant::now();
    f();
    let s = t.elapsed().as_secs_f64().max(1e-9);
    (moved as f64) / s / 1e9
}

fn disk_bench(bytes: usize, path: Option<PathBuf>, notes: &mut Vec<String>) -> DiskBench {
    let path = path.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("aida-disk-bench-{}.bin", std::process::id()))
    });
    let _ = fs::remove_file(&path);
    let chunk = vec![0xA5u8; 1024 * 1024];
    let mut written = 0usize;

    let t_write = Instant::now();
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .expect("无法创建磁盘测试文件");
        while written < bytes {
            let n = (bytes - written).min(chunk.len());
            f.write_all(&chunk[..n]).ok();
            written += n;
        }
        let t_sync = Instant::now();
        let _ = f.sync_all();
        let fsync_ms = t_sync.elapsed().as_millis();
        let write_s = t_write.elapsed().as_secs_f64().max(1e-9);
        drop(f);

        let t_read = Instant::now();
        let mut f = File::open(&path).expect("无法读回磁盘测试文件");
        let mut buf = vec![0u8; chunk.len()];
        let mut read_n = 0usize;
        loop {
            match f.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => read_n += n,
                Err(_) => break,
            }
        }
        let read_s = t_read.elapsed().as_secs_f64().max(1e-9);
        let _ = fs::remove_file(&path);
        notes.push(format!(
            "磁盘测试文件 {}，写 {} 读 {} 字节。未使用 O_DIRECT，读可能命中 page cache。",
            path.display(),
            written,
            read_n
        ));
        return DiskBench {
            path: path.display().to_string(),
            bytes: written,
            write_mbs: (written as f64) / write_s / 1e6,
            read_mbs: (read_n as f64) / read_s / 1e6,
            fsync_ms,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_bench_runs() {
        let mut req = BenchRequest::quick();
        req.disk = true;
        let r = run(&req);
        assert!(r.cpu.unwrap().score > 0.0);
        assert!(r.memory.unwrap().copy_gbs > 0.0);
        assert!(r.disk.unwrap().write_mbs > 0.0);
    }
}
