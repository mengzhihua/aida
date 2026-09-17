//! 提权重启：用户显式请求时才替换进程。采集路径绝不自动 sudo。
//!
//! 优先 `pkexec`（桌面 PolicyKit 对话框，可保留 DISPLAY），否则 `sudo -E`。
//! 策略文件见 `packaging/polkit/com.aida.linux.policy`。

use std::env;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElevateMethod {
    AlreadyRoot,
    Pkexec,
    Sudo,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElevatePlan {
    pub method: ElevateMethod,
    pub helper: Option<String>,
    pub summary: String,
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn plan() -> ElevatePlan {
    if is_root() {
        return ElevatePlan {
            method: ElevateMethod::AlreadyRoot,
            helper: None,
            summary: "已经是 root，无需再提权。".into(),
        };
    }
    if let Some(p) = find_bin(&["/usr/bin/pkexec", "/usr/local/bin/pkexec"], "pkexec") {
        return ElevatePlan {
            method: ElevateMethod::Pkexec,
            helper: Some(p.display().to_string()),
            summary: "可用 pkexec 弹出管理员认证；DISPLAY/Wayland 变量会一并传入。".into(),
        };
    }
    if let Some(p) = find_bin(&["/usr/bin/sudo", "/usr/local/bin/sudo"], "sudo") {
        return ElevatePlan {
            method: ElevateMethod::Sudo,
            helper: Some(p.display().to_string()),
            summary: "未找到 pkexec，将使用 sudo -E。无 TTY 时可能无法输入密码，请安装 pkexec 或在终端运行。".into(),
        };
    }
    ElevatePlan {
        method: ElevateMethod::Unavailable,
        helper: None,
        summary: "找不到 pkexec 或 sudo，无法提权。请用发行版软件包安装 policykit-1（Debian 包名 pkexec）或 sudo。".into(),
    }
}

/// 用管理员身份重新执行当前二进制。成功时不返回（exec 替换进程）。
pub fn reexec(args: &[String]) -> Result<(), String> {
    let plan = plan();
    let exe = env::current_exe().map_err(|e| format!("无法解析当前可执行文件: {e}"))?;
    match plan.method {
        ElevateMethod::AlreadyRoot => {
            let mut cmd = Command::new(&exe);
            cmd.args(args);
            let err = cmd.exec();
            Err(format!("exec 失败: {err}"))
        }
        ElevateMethod::Pkexec => {
            let helper = plan.helper.ok_or_else(|| "pkexec 路径丢失".to_string())?;
            let mut cmd = Command::new(helper);
            cmd.arg("env");
            for key in [
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "XAUTHORITY",
                "XDG_RUNTIME_DIR",
                "LANG",
                "LC_ALL",
                "XDG_SESSION_TYPE",
            ] {
                if let Ok(v) = env::var(key) {
                    cmd.arg(format!("{key}={v}"));
                }
            }
            cmd.arg(exe.as_os_str());
            cmd.args(args);
            let err = cmd.exec();
            Err(format!("pkexec exec 失败: {err}"))
        }
        ElevateMethod::Sudo => {
            let helper = plan.helper.ok_or_else(|| "sudo 路径丢失".to_string())?;
            let mut cmd = Command::new(helper);
            cmd.arg("-E");
            cmd.arg("--");
            cmd.arg(exe.as_os_str());
            cmd.args(args);
            let err = cmd.exec();
            Err(format!("sudo exec 失败: {err}"))
        }
        ElevateMethod::Unavailable => Err(plan.summary),
    }
}

fn find_bin(abs: &[&str], name: &str) -> Option<PathBuf> {
    for p in abs {
        if Path::new(p).is_file() {
            return Some(PathBuf::from(p));
        }
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let p = dir.join(name);
            p.is_file().then_some(p)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_coherent() {
        let p = plan();
        match p.method {
            ElevateMethod::AlreadyRoot => assert!(is_root()),
            ElevateMethod::Pkexec | ElevateMethod::Sudo => {
                assert!(!is_root());
                assert!(p.helper.is_some());
            }
            ElevateMethod::Unavailable => assert!(!is_root()),
        }
    }
}
