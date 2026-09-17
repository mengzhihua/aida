//! 命令行入口。默认启动 GUI（启用 `gui` feature 时）；无显示器可用 `collect`。
//!
//! ```text
//! aida                 # GUI（无 DISPLAY 时退化为 collect）
//! aida collect         # 打印 JSON
//! aida collect --html report.html
//! aida bench --quick
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use aida::bench::{self, BenchRequest};
use aida::export;
use aida::snapshot::HardwareSnapshot;
use aida::ProbeCtx;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("collect") => cmd_collect(&args[1..]),
        Some("bench") => cmd_bench(&args[1..]),
        Some("gui") => cmd_gui(),
        Some("elevate") => cmd_elevate(&args[1..]),
        Some("version") | Some("--version") | Some("-V") => {
            println!("aida {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        None => {
            if cfg!(feature = "gui")
                && (env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some())
            {
                cmd_gui()
            } else {
                eprintln!("无图形会话，输出 JSON 快照。GUI 请在桌面下运行 `aida gui`。");
                cmd_collect(&[])
            }
        }
        Some(other) => {
            eprintln!("未知命令: {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    eprintln!(
        "\
AIDA Linux — 硬件检测与监控（sysfs/procfs，不调用 dmidecode/lspci）

用法:
  aida                 有 DISPLAY 时启动 GUI，否则 collect
  aida gui             启动 egui 界面
  aida collect         采集快照，默认打印 JSON
  aida collect --html [FILE]
  aida collect --json [FILE]
  aida bench [--quick] [--cpu] [--memory] [--disk] [--no-direct]
  aida elevate [gui|collect|bench ...]   通过 pkexec/sudo 提权重启
  aida version

权限:
  普通用户可读 CPU、大部分 PCI、块设备容量、os-release、DRM 公开节点、
  /sys/class/net、USB sysfs、/proc/bus/input/devices、NUMA node、meminfo、
  CPU vulnerabilities、/proc/diskstats、mountinfo、/proc/modules、clocksource、
  loadavg、PSI、/proc/interrupts、virtio、buddyinfo/vmstat/KSM、PTP、block queue、
  net queues/sockstat、scsi_host、mdstat、watchdog/backlight/leds/i2c、LSM、
  zram/zswap、/dev/kvm、SMT、iSCSI transport、rfkill/蓝牙/V4L/MMC、
  snmp/softnet、cgroup v2、file-nr、zoneinfo、ext4 sysfs、consoles、
  lockdown/kptr/dmesg、/proc/crypto、namespaces、conntrack、tcp congestion、
  scsi_device、loop backing_file、ttyS、/sys/power、snmp6、aio/inotify、boot_id。
  RAPL 在无 powercap 时为空。PCIe current_link_* 在非 PCIe/虚拟桥上常不存在。
  EDAC 在未开 CONFIG_EDAC 时不存在；/proc/iomem 地址常需 root。
  ATA/SATA 仅在有 ata_port 时出现。TPM/hwrng/ACPI 表名通常可读。
  电源 serial、DMI 序列号、NVMe SMART 通常需要 root。
  DMI 序列号/UUID、SMBIOS 表、NVMe SMART、部分 USB serial 通常需要 root 或 disk 组。
  桌面请用 `aida elevate gui`（pkexec），不要对 GUI 裸 sudo 以免丢掉 DISPLAY。
  缺权限时字段标记为 permission_denied，不会伪造数据。
  GUI 传感器越限会追加 JSONL 到 $AIDA_ALERT_LOG 或 ~/.local/state/aida/alerts.jsonl。"
    );
}

fn cmd_collect(args: &[String]) -> ExitCode {
    let ctx = ProbeCtx::live();
    let snap = HardwareSnapshot::collect(&ctx);
    let mut json_path: Option<PathBuf> = None;
    let mut html_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_path = Some(next_file(args, &mut i, "aida-report.json"));
            }
            "--html" => {
                html_path = Some(next_file(args, &mut i, "aida-report.html"));
            }
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("未知参数: {other}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }

    if json_path.is_none() && html_path.is_none() {
        match export::to_json_pretty(&snap) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        }
    } else {
        if let Some(p) = json_path {
            match export::to_json_pretty(&snap) {
                Ok(s) => {
                    if let Err(e) = fs::write(&p, s) {
                        eprintln!("写 {}: {e}", p.display());
                        return ExitCode::from(1);
                    }
                    eprintln!("JSON -> {}", p.display());
                }
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            }
        }
        if let Some(p) = html_path {
            if let Err(e) = fs::write(&p, export::to_html(&snap)) {
                eprintln!("写 {}: {e}", p.display());
                return ExitCode::from(1);
            }
            eprintln!("HTML -> {}", p.display());
        }
        ExitCode::SUCCESS
    }
}

fn next_file(args: &[String], i: &mut usize, default: &str) -> PathBuf {
    if let Some(n) = args.get(*i + 1) {
        if !n.starts_with('-') {
            *i += 1;
            return PathBuf::from(n);
        }
    }
    PathBuf::from(default)
}

fn cmd_bench(args: &[String]) -> ExitCode {
    let mut req = BenchRequest::default();
    let mut specified = false;
    for a in args {
        match a.as_str() {
            "--quick" => req = BenchRequest::quick(),
            "--cpu" => {
                if !specified {
                    req.cpu = false;
                    req.memory = false;
                    req.disk = false;
                    specified = true;
                }
                req.cpu = true;
            }
            "--memory" => {
                if !specified {
                    req.cpu = false;
                    req.memory = false;
                    req.disk = false;
                    specified = true;
                }
                req.memory = true;
            }
            "--disk" => {
                if !specified {
                    req.cpu = false;
                    req.memory = false;
                    req.disk = false;
                    specified = true;
                }
                req.disk = true;
            }
            "--no-direct" => req.o_direct = false,
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("未知参数: {other}");
                return ExitCode::from(2);
            }
        }
    }
    eprintln!(
        "运行微基准（线程={}）…",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    );
    let report = bench::run(&req);
    match serde_json::to_string_pretty(&report) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_elevate(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    let rest = if args.is_empty() {
        vec!["gui".into()]
    } else {
        args.to_vec()
    };
    if rest.first().map(|s| s.as_str()) == Some("elevate") {
        eprintln!("拒绝嵌套 elevate");
        return ExitCode::from(2);
    }
    let plan = aida::elevate::plan();
    eprintln!("{}", plan.summary);
    match aida::elevate::reexec(&rest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_gui() -> ExitCode {
    #[cfg(feature = "gui")]
    {
        match aida::ui::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("GUI 启动失败: {e}");
                eprintln!("可改用: aida collect");
                ExitCode::from(1)
            }
        }
    }
    #[cfg(not(feature = "gui"))]
    {
        eprintln!(
            "此二进制未启用 gui feature。请 `cargo build --features gui` 或使用 aida collect。"
        );
        ExitCode::from(1)
    }
}
