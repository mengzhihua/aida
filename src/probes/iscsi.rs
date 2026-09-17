//! iSCSI：`/sys/class/iscsi_{transport,host,session}`。不调用 `iscsiadm`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct IscsiReport {
    pub transports: Vec<IscsiTransport>,
    pub hosts: Vec<IscsiHost>,
    pub sessions: Vec<IscsiSession>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IscsiTransport {
    pub name: String,
    pub handle: Sample<String>,
    pub caps: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IscsiHost {
    pub name: String,
    pub hwaddress: Sample<String>,
    pub ipaddress: Sample<String>,
    pub netdev: Sample<String>,
    pub port_state: Sample<String>,
    pub port_speed: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IscsiSession {
    pub name: String,
    pub targetname: Sample<String>,
    pub state: Sample<String>,
    pub initiatorname: Sample<String>,
    pub ifacename: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> IscsiReport {
    let mut notes = Vec::new();
    let transports = read_transports(ctx, &mut notes);
    let hosts = read_hosts(ctx, &mut notes);
    let sessions = read_sessions(ctx, &mut notes);
    if transports.is_empty() && hosts.is_empty() && sessions.is_empty() && notes.is_empty() {
        notes.push("无 iSCSI 传输/会话（未用软件 iSCSI 时正常）。".into());
    }
    IscsiReport {
        transports,
        hosts,
        sessions,
        notes,
    }
}

fn list_class(ctx: &ProbeCtx, class: &str, notes: &mut Vec<String>) -> Vec<String> {
    let root = ctx.sys_path(format!("class/{class}"));
    match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s if s.access == AccessKind::NotFound => Vec::new(),
        s => {
            notes.push(s.access_label());
            Vec::new()
        }
    }
}

fn read_transports(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<IscsiTransport> {
    let root = ctx.sys_path("class/iscsi_transport");
    let mut out = Vec::new();
    for name in list_class(ctx, "iscsi_transport", notes) {
        let dir = root.join(&name);
        out.push(IscsiTransport {
            handle: access::read_trimmed(dir.join("handle")),
            caps: access::read_trimmed(dir.join("caps")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_hosts(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<IscsiHost> {
    let root = ctx.sys_path("class/iscsi_host");
    let mut out = Vec::new();
    for name in list_class(ctx, "iscsi_host", notes).into_iter().filter(|n| n.starts_with("host")) {
        let dir = root.join(&name);
        out.push(IscsiHost {
            hwaddress: access::read_trimmed(dir.join("hwaddress")),
            ipaddress: access::read_trimmed(dir.join("ipaddress")),
            netdev: access::read_trimmed(dir.join("netdev")),
            port_state: access::read_trimmed(dir.join("port_state")),
            port_speed: access::read_trimmed(dir.join("port_speed")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_sessions(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<IscsiSession> {
    let root = ctx.sys_path("class/iscsi_session");
    let mut out = Vec::new();
    for name in list_class(ctx, "iscsi_session", notes)
        .into_iter()
        .filter(|n| n.starts_with("session"))
    {
        let dir = root.join(&name);
        out.push(IscsiSession {
            targetname: access::read_trimmed(dir.join("targetname")),
            state: access::read_trimmed(dir.join("state")),
            initiatorname: access::read_trimmed(dir.join("initiatorname")),
            ifacename: access::read_trimmed(dir.join("ifacename")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn transport_and_session_fixture() {
        let root = std::env::temp_dir().join(format!("aida-iscsi-{}", std::process::id()));
        let tr = root.join("sys/class/iscsi_transport/tcp");
        fs::create_dir_all(&tr).unwrap();
        fs::write(tr.join("handle"), "1\n").unwrap();
        let sess = root.join("sys/class/iscsi_session/session1");
        fs::create_dir_all(&sess).unwrap();
        fs::write(sess.join("targetname"), "iqn.2020-01.com.example:disk\n").unwrap();
        fs::write(sess.join("state"), "LOGGED_IN\n").unwrap();
        fs::create_dir_all(root.join("sys/class/iscsi_host")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.transports[0].name, "tcp");
        assert_eq!(r.sessions[0].state.value.as_deref(), Some("LOGGED_IN"));
        assert!(r.sessions[0].targetname.value.as_deref().unwrap().contains("iqn."));
        let _ = fs::remove_dir_all(&root);
    }
}
