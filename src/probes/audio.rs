//! 声卡：`/proc/asound/cards` + `/sys/class/sound`。不调用 `aplay`/`pactl`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct AudioReport {
    pub cards: Vec<SoundCard>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SoundCard {
    pub index: u32,
    pub id: String,
    pub name: String,
    pub extra: Option<String>,
    pub sys_id: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> AudioReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("asound/cards"));
    let mut cards = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_asound_cards(text),
        _ => {
            notes.push(sample.access_label());
            Vec::new()
        }
    };
    for c in &mut cards {
        c.sys_id = access::read_trimmed(ctx.sys_path(format!("class/sound/card{}/id", c.index)));
    }
    if cards.is_empty() && notes.is_empty() {
        notes.push("无 ALSA 声卡（无头虚拟机/容器常见）。".into());
    }
    AudioReport { cards, notes }
}

pub fn parse_asound_cards(text: &str) -> Vec<SoundCard> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_start();
        if t.is_empty() {
            i += 1;
            continue;
        }
        let mut parts = t.splitn(2, ' ');
        let idx_s = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");
        if let Ok(index) = idx_s.parse::<u32>() {
            let id = rest
                .split_once('[')
                .and_then(|(_, r)| r.split_once(']'))
                .map(|(id, _)| id.trim().to_string())
                .unwrap_or_default();
            let name = rest
                .split("]: ")
                .nth(1)
                .unwrap_or(rest)
                .trim()
                .to_string();
            let extra = lines.get(i + 1).and_then(|n| {
                let s = n.trim();
                if s.is_empty() || s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    None
                } else {
                    i += 1;
                    Some(s.to_string())
                }
            });
            out.push(SoundCard {
                index,
                id,
                name,
                extra,
                sys_id: Sample::missing("class/sound"),
            });
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_two_cards() {
        let text = "\
 0 [PCH            ]: HDA-Intel - HDA Intel PCH
                      HDA Intel PCH at 0xf7110000 irq 129
 1 [HDMI           ]: HDA-Intel - HDA ATI HDMI
                      HDA ATI HDMI at 0xf7a60000 irq 130
";
        let c = parse_asound_cards(text);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].index, 0);
        assert_eq!(c[0].id, "PCH");
        assert!(c[0].name.contains("HDA Intel PCH"));
        assert!(c[0].extra.as_deref().unwrap().contains("0xf7110000"));
        assert_eq!(c[1].id, "HDMI");
    }
}
