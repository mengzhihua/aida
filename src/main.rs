//! 命令行入口。默认启动 GUI（启用 `gui` feature 时）；无显示器可用 `collect`。
//!
//! ```text
//! aida                 # GUI（无 DISPLAY 时退化为 collect）
//! aida collect         # 打印 JSON
//! aida collect [--json FILE] [--html FILE] [--text FILE] [--csv FILE] [--md FILE]
//! aida collect --format text   # 打印可读报告（默认仍是 JSON）
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
        Some("doctor") => cmd_doctor(&args[1..]),
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
AIDA Linux {} — 硬件检测与监控（只读 /proc /sys /dev，不调用 dmidecode/lspci）

用法:
  aida                              有图形会话则 GUI，否则打印 JSON
  aida gui                          桌面界面（状态栏 / 传感器折线 / 导出）
  aida collect [--format json|html|text|csv|md]
               [--json FILE] [--html FILE] [--text FILE] [--csv FILE] [--md FILE]
               FILE 为 - 时写到 stdout。一次采集可同时写出多种格式。
  aida doctor [--json]
  aida bench [--quick] [--cpu] [--memory] [--disk] [--no-direct]
  aida elevate [gui|collect|bench ...]   pkexec，没有则 sudo -E
  aida version

环境:
  AIDA_ALERT_LOG    传感器越限 JSONL（默认 ~/.local/state/aida/alerts.jsonl）
  AIDA_RECORD_LOG   状态栏历史 JSONL（默认 ~/.local/state/aida/history.jsonl）

权限: 缺权限标 permission_denied，不填假数据。DMI 序列号 / NVMe SMART 通常要
root 或 disk 组。桌面提权用 `aida elevate gui`，不要对 GUI 裸 sudo 以免丢掉 DISPLAY。

打包: 解压 tar.gz 后 ./install.sh（Ubuntu/Debian 与 CentOS/RHEL 都能装）。源码 ./scripts/package.sh
详见 README.md 与 docs/PACKAGING.md。",
        env!("CARGO_PKG_VERSION")
    );
}

fn cmd_collect(args: &[String]) -> ExitCode {
    let ctx = ProbeCtx::live();
    let snap = HardwareSnapshot::collect(&ctx);
    let mut stdout_fmt = export::ReportFormat::Json;
    let mut stdout_explicit = false;
    let mut files: Vec<(export::ReportFormat, PathBuf)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                let Some(name) = args.get(i + 1) else {
                    eprintln!("--format 需要 json / html / text / csv / md");
                    return ExitCode::from(2);
                };
                let Some(fmt) = export::ReportFormat::parse(name) else {
                    eprintln!("未知 --format {name}（json / html / text / csv / md）");
                    return ExitCode::from(2);
                };
                stdout_fmt = fmt;
                stdout_explicit = true;
                i += 1;
            }
            "--json" => files.push((
                export::ReportFormat::Json,
                next_file(args, &mut i, "aida-report.json"),
            )),
            "--html" => files.push((
                export::ReportFormat::Html,
                next_file(args, &mut i, "aida-report.html"),
            )),
            "--text" | "--txt" => files.push((
                export::ReportFormat::Text,
                next_file(args, &mut i, "aida-report.txt"),
            )),
            "--csv" => files.push((
                export::ReportFormat::Csv,
                next_file(args, &mut i, "aida-report.csv"),
            )),
            "--md" | "--markdown" => files.push((
                export::ReportFormat::Markdown,
                next_file(args, &mut i, "aida-report.md"),
            )),
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

    let print_stdout = files.is_empty() || stdout_explicit;
    if print_stdout {
        match stdout_fmt.render(&snap) {
            Ok(s) => emit_stdout(&s),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        }
        if files.is_empty() {
            return ExitCode::SUCCESS;
        }
    }

    for (fmt, p) in &files {
        let body = match fmt.render(&snap) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        };
        if p.as_os_str() == "-" {
            if !print_stdout {
                emit_stdout(&body);
            }
            continue;
        }
        if let Err(e) = fs::write(p, &body) {
            eprintln!("写 {}: {e}", p.display());
            return ExitCode::from(1);
        }
        eprintln!("{} -> {}", format_label(*fmt), p.display());
    }
    ExitCode::SUCCESS
}

fn emit_stdout(s: &str) {
    print!("{s}");
    if !s.ends_with('\n') {
        println!();
    }
}

fn format_label(fmt: export::ReportFormat) -> &'static str {
    match fmt {
        export::ReportFormat::Json => "JSON",
        export::ReportFormat::Html => "HTML",
        export::ReportFormat::Text => "TEXT",
        export::ReportFormat::Csv => "CSV",
        export::ReportFormat::Markdown => "MD",
    }
}

fn cmd_doctor(args: &[String]) -> ExitCode {
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "-h" | "--help" => {
                eprintln!(
                    "aida doctor [--json]   检查发行版、glibc、GUI 库，给出 apt/dnf/yum 安装命令"
                );
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("未知参数: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let ctx = ProbeCtx::live();
    let report = aida::doctor::collect(&ctx);
    if json {
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
    } else {
        print!("{}", aida::doctor::format_text(&report));
        ExitCode::SUCCESS
    }
}

fn next_file(args: &[String], i: &mut usize, default: &str) -> PathBuf {
    if let Some(n) = args.get(*i + 1) {
        if n == "-" || !n.starts_with('-') {
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
