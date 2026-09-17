//! 输入设备：`/proc/bus/input/devices`，不调用 `libinput list-devices`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct InputReport {
    pub devices: Vec<InputDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InputDevice {
    pub name: String,
    pub phys: Option<String>,
    pub sysfs: Option<String>,
    pub uniq: Option<String>,
    pub handlers: Vec<String>,
    pub bustype: Option<String>,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub ev_bits: Option<String>,
    pub kinds: Vec<String>,
}

pub fn collect(ctx: &ProbeCtx) -> InputReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("bus/input/devices"));
    let devices = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_input_devices(text),
        (AccessKind::Ok, None) => {
            notes.push(
                sample
                    .hint
                    .clone()
                    .unwrap_or_else(|| "输入设备表为空（无头环境常见）。".into()),
            );
            Vec::new()
        }
        _ => {
            notes.push(sample.access_label());
            Vec::new()
        }
    };
    if devices.is_empty() && notes.is_empty() {
        notes.push("输入设备表为空（无头环境常见）。".into());
    }
    InputReport { devices, notes }
}

pub fn parse_input_devices(text: &str) -> Vec<InputDevice> {
    let mut out = Vec::new();
    let mut cur = InputDevice {
        name: String::new(),
        phys: None,
        sysfs: None,
        uniq: None,
        handlers: Vec::new(),
        bustype: None,
        vendor: None,
        product: None,
        ev_bits: None,
        kinds: Vec::new(),
    };
    let mut started = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            if started {
                finalize(&mut cur);
                out.push(std::mem::replace(
                    &mut cur,
                    InputDevice {
                        name: String::new(),
                        phys: None,
                        sysfs: None,
                        uniq: None,
                        handlers: Vec::new(),
                        bustype: None,
                        vendor: None,
                        product: None,
                        ev_bits: None,
                        kinds: Vec::new(),
                    },
                ));
                started = false;
            }
            continue;
        }
        started = true;
        if let Some(rest) = line.strip_prefix("N: Name=") {
            cur.name = rest.trim().trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("P: Phys=") {
            cur.phys = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("S: Sysfs=") {
            cur.sysfs = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("U: Uniq=") {
            let u = rest.trim();
            if !u.is_empty() {
                cur.uniq = Some(u.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
            cur.handlers = rest.split_whitespace().map(|s| s.to_string()).collect();
        } else if let Some(rest) = line.strip_prefix("I: ") {
            for part in rest.split_whitespace() {
                if let Some(v) = part.strip_prefix("Bus=") {
                    cur.bustype = Some(v.to_string());
                } else if let Some(v) = part.strip_prefix("Vendor=") {
                    cur.vendor = Some(v.to_string());
                } else if let Some(v) = part.strip_prefix("Product=") {
                    cur.product = Some(v.to_string());
                }
            }
        } else if let Some(rest) = line.strip_prefix("B: EV=") {
            cur.ev_bits = Some(rest.trim().to_string());
        }
    }
    if started {
        finalize(&mut cur);
        out.push(cur);
    }
    out
}

fn finalize(dev: &mut InputDevice) {
    let mut kinds = Vec::new();
    for h in &dev.handlers {
        if h.starts_with("kbd") || *h == "kbd" {
            kinds.push("keyboard".into());
        } else if h.starts_with("mouse") {
            kinds.push("mouse".into());
        } else if h.starts_with("js") {
            kinds.push("joystick".into());
        }
    }
    if let Some(ev) = &dev.ev_bits {
        // EV_KEY=0x01 bit0, EV_REL=0x02 bit1, EV_ABS=0x03 bit2 — hex bitmask
        if let Ok(bits) = u64::from_str_radix(ev, 16) {
            if bits & (1 << 1) != 0 {
                kinds.push("key".into());
            }
            if bits & (1 << 2) != 0 {
                kinds.push("rel".into());
            }
            if bits & (1 << 3) != 0 {
                kinds.push("abs".into());
            }
            if bits & (1 << 5) != 0 {
                kinds.push("sw".into());
            }
        }
    }
    kinds.sort();
    kinds.dedup();
    dev.kinds = kinds;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keyboard_block() {
        let text = "\
I: Bus=0011 Vendor=0001 Product=0001 Version=ab41
N: Name=\"AT Translated Set 2 keyboard\"
P: Phys=isa0060/serio0/input0
S: Sysfs=/devices/platform/i8042/serio0/input/input0
U: Uniq=
H: Handlers=sysrq kbd event0
B: PROP=0
B: EV=120013

";
        let d = parse_input_devices(text);
        assert_eq!(d.len(), 1);
        assert!(d[0].name.contains("keyboard"));
        assert!(d[0].handlers.iter().any(|h| h == "kbd"));
        assert!(d[0].kinds.contains(&"keyboard".to_string()));
    }
}
