//! 时钟：clocksource + RTC。不调用 `hwclock`/`timedatectl`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct ClockReport {
    pub current: Sample<String>,
    pub available: Sample<String>,
    pub rtcs: Vec<Rtc>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Rtc {
    pub name: String,
    pub rtc_name: Sample<String>,
    pub date: Sample<String>,
    pub since_epoch: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> ClockReport {
    let mut notes = Vec::new();
    let cs = ctx.sys_path("devices/system/clocksource/clocksource0");
    let current = access::read_trimmed(cs.join("current_clocksource"));
    let available = access::read_trimmed(cs.join("available_clocksource"));
    if current.access != AccessKind::Ok {
        notes.push(current.access_label());
    }
    let rtc_root = ctx.sys_path("class/rtc");
    let mut rtcs = Vec::new();
    match access::list_dir_names(&rtc_root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(names),
            ..
        } => {
            for name in names.into_iter().filter(|n| n.starts_with("rtc")) {
                let dir = rtc_root.join(&name);
                rtcs.push(Rtc {
                    rtc_name: access::read_trimmed(dir.join("name")),
                    date: access::read_trimmed(dir.join("date")),
                    since_epoch: access::read_trimmed(dir.join("since_epoch")),
                    name,
                });
            }
        }
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            }
        }
    }
    if rtcs.is_empty() {
        notes.push("无 RTC 设备（虚拟机/容器常见）。".into());
    }
    ClockReport {
        current,
        available,
        rtcs,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn kvm_clock_fixture() {
        let root = std::env::temp_dir().join(format!("aida-clk-{}", std::process::id()));
        let cs = root.join("sys/devices/system/clocksource/clocksource0");
        fs::create_dir_all(&cs).unwrap();
        fs::write(cs.join("current_clocksource"), "kvm-clock\n").unwrap();
        fs::write(cs.join("available_clocksource"), "kvm-clock tsc\n").unwrap();
        let rtc = root.join("sys/class/rtc/rtc0");
        fs::create_dir_all(&rtc).unwrap();
        fs::write(rtc.join("name"), "rtc-test\n").unwrap();
        fs::write(rtc.join("date"), "2026-09-17\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.current.value.as_deref(), Some("kvm-clock"));
        assert_eq!(r.rtcs.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
