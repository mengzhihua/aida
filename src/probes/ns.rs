//! 命名空间：`/proc/self/ns` + `/proc/sys/user/max_*_namespaces`。
//! 不调用 `lsns`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct NsReport {
    pub self_ns: Vec<NsLink>,
    pub max_user: Sample<u64>,
    pub max_pid: Sample<u64>,
    pub max_mnt: Sample<u64>,
    pub max_net: Sample<u64>,
    pub max_uts: Sample<u64>,
    pub max_ipc: Sample<u64>,
    pub max_cgroup: Sample<u64>,
    pub max_time: Sample<u64>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NsLink {
    pub kind: String,
    pub inode: Option<u64>,
    pub target: String,
}

pub fn collect(ctx: &ProbeCtx) -> NsReport {
    let mut notes = Vec::new();
    let self_ns = read_self_ns(ctx);
    if self_ns.is_empty() {
        notes.push("无法读取 /proc/self/ns（容器未挂 proc 时常见）。".into());
    }
    let user = ctx.proc_path("sys/user");
    NsReport {
        self_ns,
        max_user: access::read_u64(user.join("max_user_namespaces")),
        max_pid: access::read_u64(user.join("max_pid_namespaces")),
        max_mnt: access::read_u64(user.join("max_mnt_namespaces")),
        max_net: access::read_u64(user.join("max_net_namespaces")),
        max_uts: access::read_u64(user.join("max_uts_namespaces")),
        max_ipc: access::read_u64(user.join("max_ipc_namespaces")),
        max_cgroup: access::read_u64(user.join("max_cgroup_namespaces")),
        max_time: access::read_u64(user.join("max_time_namespaces")),
        notes,
    }
}

fn read_self_ns(ctx: &ProbeCtx) -> Vec<NsLink> {
    let dir = ctx.proc_path("self/ns");
    let names = match access::list_dir_names(&dir) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for kind in names {
        if kind.ends_with("_for_children") {
            continue;
        }
        let path = dir.join(&kind);
        match std::fs::read_link(&path) {
            Ok(t) => {
                let target = t.to_string_lossy().into_owned();
                let inode = parse_ns_inode(&target);
                out.push(NsLink {
                    kind,
                    inode,
                    target,
                });
            }
            Err(_) => out.push(NsLink {
                kind,
                inode: None,
                target: String::new(),
            }),
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind));
    out
}

/// `net:[4026531840]` → inode。
pub fn parse_ns_inode(target: &str) -> Option<u64> {
    let start = target.find('[')? + 1;
    let end = target.find(']')?;
    target[start..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn parses_ns_target() {
        assert_eq!(parse_ns_inode("net:[4026531840]"), Some(4026531840));
        assert_eq!(parse_ns_inode("broken"), None);
    }

    #[test]
    fn collect_self_ns_fixture() {
        let root = std::env::temp_dir().join(format!("aida-ns-{}", std::process::id()));
        let ns = root.join("proc/self/ns");
        fs::create_dir_all(&ns).unwrap();
        fs::create_dir_all(root.join("proc/sys/user")).unwrap();
        symlink("net:[4026531840]", ns.join("net")).unwrap();
        symlink("pid:[1]", ns.join("pid")).unwrap();
        symlink("pid:[1]", ns.join("pid_for_children")).unwrap();
        fs::write(root.join("proc/sys/user/max_user_namespaces"), "64035\n").unwrap();
        fs::write(root.join("proc/sys/user/max_pid_namespaces"), "64035\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.max_user.value, Some(64035));
        assert_eq!(r.self_ns.len(), 2);
        assert_eq!(r.self_ns.iter().find(|n| n.kind == "net").unwrap().inode, Some(4026531840));
        assert!(!r.self_ns.iter().any(|n| n.kind.contains("for_children")));
        let _ = fs::remove_dir_all(&root);
    }
}
