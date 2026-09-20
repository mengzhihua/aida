//! 发行版体检：读 os-release、libc、GUI `.so`，给出 apt / dnf / yum 安装提示。
//! 不调用 `lsb_release`、`ldd`、`hostnamectl`。

use std::path::PathBuf;

use serde::Serialize;

use crate::access::{self, ProbeCtx, Sample};
use crate::probes::software::{distro_family, parse_os_release_fields};

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub os_name: Sample<String>,
    pub os_id: Sample<String>,
    pub os_like: Sample<String>,
    pub os_version: Sample<String>,
    /// `debian` / `rhel` / `suse` / `arch` / `unknown`
    pub family: String,
    /// 本机 libc 能提供的最高 `GLIBC_x.y`；musl 为 `None`。
    pub glibc: Option<String>,
    pub musl: bool,
    pub display: bool,
    pub wayland: bool,
    pub gui_libs: Vec<GuiLib>,
    pub gui_ok: bool,
    /// 当前包桌面 GUI 需要的最低 glibc（包内 `GLIBC_GUI`，否则 `GUI_GLIBC_HINT`）。
    pub gui_need_glibc: String,
    pub hints: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuiLib {
    pub name: String,
    pub path: Option<String>,
}

const GUI_LIBS: &[&str] = &[
    "libEGL.so.1",
    "libGL.so.1",
    "libxkbcommon.so.0",
    "libxkbcommon-x11.so.0",
];

/// Ubuntu 24.04 上编的 glibc GUI/AppImage 会用到的符号上限（见 `packaging/GLIBC_GUI`）。
pub const GUI_GLIBC_HINT: &str = "2.39";

pub fn collect(ctx: &ProbeCtx) -> DoctorReport {
    collect_with_lib_roots(ctx, &default_lib_roots())
}

pub fn collect_with_lib_roots(ctx: &ProbeCtx, lib_roots: &[PathBuf]) -> DoctorReport {
    let os = parse_os_release_fields(&access::read_trimmed(ctx.etc.join("os-release")));
    let id = os.id.value.clone().unwrap_or_default();
    let like = os.id_like.value.clone().unwrap_or_default();
    let family = distro_family(&id, &like).to_string();

    // musl-tools 装在 Ubuntu 上也会有 /lib/ld-musl-*；有 GLIBC_ 符号才算主 libc。
    let glibc = scan_glibc(lib_roots);
    let musl = glibc.is_none() && has_musl_loader(lib_roots);

    let mut gui_libs = Vec::new();
    let mut missing = Vec::new();
    for name in GUI_LIBS {
        match find_so(lib_roots, name) {
            Some(p) => gui_libs.push(GuiLib {
                name: (*name).to_string(),
                path: Some(p),
            }),
            None => {
                gui_libs.push(GuiLib {
                    name: (*name).to_string(),
                    path: None,
                });
                missing.push(*name);
            }
        }
    }
    let gui_ok = missing.is_empty();

    let display = std::env::var_os("DISPLAY").is_some();
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let gui_need_glibc = load_gui_need_glibc();

    let mut hints = Vec::new();
    hints.push(format!(
        "发行版 family={family}（Ubuntu/Debian 走 apt，CentOS/RHEL/Rocky/Fedora 走 dnf/yum）"
    ));
    if musl {
        hints.push("本机是 musl。采集用随包 aida-cli 即可；桌面 GUI 请用 glibc AppImage 或在 glibc 发行版上跑。".into());
    } else if let Some(ref g) = glibc {
        if glibc_less(g, &gui_need_glibc) {
            hints.push(format!(
                "本机 glibc {g} 低于当前包桌面 GUI 所需 {gui_need_glibc}。桌面 GUI 可能无法启动；采集请用 musl 静态 CLI（CentOS 7 / Ubuntu 16.04 也能跑）。"
            ));
        } else {
            hints.push(format!(
                "本机 glibc {g}，对照本包 GUI 目标 {gui_need_glibc}，桌面 AppImage 通常可运行。"
            ));
        }
    }
    if !gui_ok {
        hints.push(format!(
            "缺少 GUI 库 {}。安装：{}",
            missing.join(", "),
            gui_install_cmd(&family)
        ));
    } else {
        hints.push("GUI 运行库已找到（AppImage 也会自带一份 .so）。".into());
    }
    if !display && !wayland {
        hints.push("没有 DISPLAY/WAYLAND_DISPLAY：用 `./aida-cli collect` 或 `./run-collect.sh`。".into());
    }
    hints.push(format!("运行库一键装：{}", gui_install_cmd(&family)));

    DoctorReport {
        os_name: os.pretty,
        os_id: os.id,
        os_like: os.id_like,
        os_version: os.version,
        family,
        glibc,
        musl,
        display,
        wayland,
        gui_libs,
        gui_ok,
        gui_need_glibc,
        hints,
    }
}

pub fn format_text(r: &DoctorReport) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "AIDA doctor  os={}  id={}  like={}  ver={}  family={}\n",
        r.os_name.display(),
        r.os_id.display(),
        r.os_like.display(),
        r.os_version.display(),
        r.family
    ));
    match (&r.glibc, r.musl) {
        (_, true) => s.push_str("libc: musl\n"),
        (Some(g), _) => s.push_str(&format!("glibc: {g}\n")),
        _ => s.push_str("glibc: ?\n"),
    }
    s.push_str(&format!("gui_need_glibc: {}\n", r.gui_need_glibc));
    s.push_str(&format!(
        "display={}  wayland={}\n",
        r.display, r.wayland
    ));
    for lib in &r.gui_libs {
        match &lib.path {
            Some(p) => s.push_str(&format!("  {}  {p}\n", lib.name)),
            None => s.push_str(&format!("  {}  MISSING\n", lib.name)),
        }
    }
    for h in &r.hints {
        s.push_str(&format!("- {h}\n"));
    }
    s
}

pub fn gui_install_cmd(family: &str) -> &'static str {
    match family {
        "debian" => "sudo apt-get install -y libxkbcommon-x11-0 libegl1 libgl1 pkexec",
        "rhel" => "sudo dnf install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit || sudo yum install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit",
        "suse" => "sudo zypper install -y libxkbcommon-x11-0 Mesa-libEGL1 Mesa-libGL1 polkit",
        "arch" => "sudo pacman -S --needed libxkbcommon mesa polkit",
        "alpine" => "sudo apk add --no-cache mesa-egl mesa-gl libxkbcommon libxkbcommon-x11 polkit",
        _ => "需要 OpenGL/EGL 与 libxkbcommon（含 x11）；发行版包名见 README",
    }
}

pub fn max_glibc_label(bytes: &[u8]) -> Option<String> {
    let mut best: Option<(u32, u32, u32)> = None;
    let mut i = 0;
    while let Some(pos) = find_sub(bytes, b"GLIBC_", i) {
        let rest = &bytes[pos + 6..];
        if let Some(v) = parse_glibc_triple(rest) {
            if best.map(|b| v > b).unwrap_or(true) {
                best = Some(v);
            }
        }
        i = pos + 1;
    }
    best.map(|(a, b, c)| {
        if c == 0 {
            format!("{a}.{b}")
        } else {
            format!("{a}.{b}.{c}")
        }
    })
}

fn parse_glibc_triple(rest: &[u8]) -> Option<(u32, u32, u32)> {
    let mut n = Vec::new();
    let mut cur = 0u32;
    let mut seen = false;
    for &ch in rest {
        if ch.is_ascii_digit() {
            seen = true;
            cur = cur.saturating_mul(10).saturating_add((ch - b'0') as u32);
        } else if ch == b'.' && seen {
            n.push(cur);
            cur = 0;
            seen = false;
            if n.len() == 2 {
                // 可能还有第三段
            }
        } else {
            if seen {
                n.push(cur);
            }
            break;
        }
    }
    if n.is_empty() {
        return None;
    }
    Some((n[0], n.get(1).copied().unwrap_or(0), n.get(2).copied().unwrap_or(0)))
}

fn glibc_less(have: &str, need: &str) -> bool {
    fn parts(s: &str) -> (u32, u32, u32) {
        let mut it = s.split('.');
        let a = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let b = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let c = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        (a, b, c)
    }
    parts(have) < parts(need)
}

fn find_sub(hay: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    hay[start..].windows(needle.len()).position(|w| w == needle).map(|p| start + p)
}

fn load_gui_need_glibc() -> String {
    for p in gui_need_glibc_paths() {
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Some(v) = parse_glibc_need_line(&text) {
                return v;
            }
        }
    }
    GUI_GLIBC_HINT.to_string()
}

fn parse_glibc_need_line(text: &str) -> Option<String> {
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    if !line
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
    {
        return None;
    }
    let mut parts = line.split('.');
    parts.next()?.parse::<u32>().ok()?;
    Some(line.to_string())
}

fn gui_need_glibc_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("GLIBC_GUI"));
            out.push(dir.join("../lib/aida/GLIBC_GUI"));
        }
    }
    out.push(PathBuf::from("GLIBC_GUI"));
    out
}

fn has_musl_loader(lib_roots: &[PathBuf]) -> bool {
    lib_roots.iter().any(|r| {
        r.join("ld-musl-x86_64.so.1").exists()
            || r.join("ld-musl-aarch64.so.1").exists()
            || r.join("ld-musl-i386.so.1").exists()
    })
}

fn default_lib_roots() -> Vec<PathBuf> {
    [
        "/lib64",
        "/usr/lib64",
        "/lib/x86_64-linux-gnu",
        "/usr/lib/x86_64-linux-gnu",
        "/lib/aarch64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib",
        "/usr/lib",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn find_so(roots: &[PathBuf], name: &str) -> Option<String> {
    for r in roots {
        let p = r.join(name);
        if p.exists() {
            return Some(p.display().to_string());
        }
    }
    None
}

fn scan_glibc(roots: &[PathBuf]) -> Option<String> {
    let names = ["libc.so.6", "libc-2.28.so", "libc-2.17.so"];
    for r in roots {
        for n in names {
            let p = r.join(n);
            if let Ok(bytes) = std::fs::read(&p) {
                if let Some(v) = max_glibc_label(&bytes) {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn glibc_max_picks_highest() {
        let blob = b"xxGLIBC_2.2.5\0GLIBC_2.39\0GLIBC_2.17\0";
        assert_eq!(max_glibc_label(blob).as_deref(), Some("2.39"));
        assert_eq!(parse_glibc_need_line("2.39\n# comment\n").as_deref(), Some("2.39"));
        assert_eq!(parse_glibc_need_line("nope"), None);
    }

    #[test]
    fn family_ubuntu_and_centos() {
        assert_eq!(distro_family("ubuntu", "debian"), "debian");
        assert_eq!(distro_family("centos", "rhel fedora"), "rhel");
        assert_eq!(distro_family("rocky", "rhel centos fedora"), "rhel");
        assert_eq!(distro_family("fedora", ""), "rhel");
        assert_eq!(distro_family("opensuse-leap", "suse opensuse"), "suse");
        assert_eq!(distro_family("arch", ""), "arch");
        assert_eq!(distro_family("alpine", ""), "alpine");
        assert!(gui_install_cmd("debian").contains("apt-get"));
        assert!(gui_install_cmd("rhel").contains("dnf"));
        assert!(gui_install_cmd("rhel").contains("yum"));
    }

    #[test]
    fn collect_centos_fixture_hints_dnf() {
        let root = std::env::temp_dir().join(format!("aida-doctor-{}", std::process::id()));
        fs::create_dir_all(root.join("etc")).unwrap();
        fs::write(
            root.join("etc/os-release"),
            "NAME=\"CentOS Stream\"\nID=\"centos\"\nID_LIKE=\"rhel fedora\"\nVERSION_ID=\"9\"\nPRETTY_NAME=\"CentOS Stream 9\"\n",
        )
        .unwrap();
        let lib = root.join("lib64");
        fs::create_dir_all(&lib).unwrap();
        fs::write(lib.join("libc.so.6"), b"GLIBC_2.34\0GLIBC_2.17\0").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect_with_lib_roots(&ctx, &[lib]);
        assert_eq!(r.os_id.value.as_deref(), Some("centos"));
        assert_eq!(r.family, "rhel");
        assert!(!r.musl, "glibc 夹具不该被宿主机 musl-gcc 判成 musl OS");
        assert_eq!(r.glibc.as_deref(), Some("2.34"));
        assert_eq!(r.gui_need_glibc.as_str(), GUI_GLIBC_HINT);
        assert!(r.hints.iter().any(|h| h.contains("dnf") || h.contains("yum")));
        assert!(
            r.hints.iter().any(|h| h.contains("低于") || h.contains("2.39")),
            "glibc 2.34 should warn vs AppImage 2.39: {:?}",
            r.hints
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn collect_ubuntu_fixture_apt() {
        let root = std::env::temp_dir().join(format!("aida-doctor-ub-{}", std::process::id()));
        fs::create_dir_all(root.join("etc")).unwrap();
        fs::write(
            root.join("etc/os-release"),
            "PRETTY_NAME=\"Ubuntu 24.04.4 LTS\"\nID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"24.04\"\n",
        )
        .unwrap();
        let lib = root.join("lib");
        fs::create_dir_all(&lib).unwrap();
        fs::write(lib.join("libc.so.6"), b"GLIBC_2.39\0").unwrap();
        fs::write(lib.join("libEGL.so.1"), b"").unwrap();
        fs::write(lib.join("libGL.so.1"), b"").unwrap();
        fs::write(lib.join("libxkbcommon.so.0"), b"").unwrap();
        fs::write(lib.join("libxkbcommon-x11.so.0"), b"").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect_with_lib_roots(&ctx, &[lib]);
        assert_eq!(r.family, "debian");
        assert!(r.gui_ok);
        assert!(gui_install_cmd(&r.family).contains("apt-get"));
        let text = format_text(&r);
        assert!(text.contains("family=debian"));
        let _ = fs::remove_dir_all(&root);
    }
}
