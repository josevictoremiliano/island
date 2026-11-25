use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Colors {
    pub busy: String,
    pub question: String,
    pub done: String,
    pub error: String,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            busy: "#ff9f43".into(),
            question: "#ff5d8f".into(),
            done: "#3ddc97".into(),
            error: "#ff4d4f".into(),
        }
    }
}

/// Which small apps appear in the island.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Apps {
    pub clock: bool,
    pub usage: bool,
    pub timer: bool,
    pub note: bool,
    pub music: bool,
    pub system: bool,
    pub clipboard: bool,
    pub shelf: bool,
    pub net: bool,
    pub links: bool,
    pub focus: bool,
    pub lyrics: bool,
    pub calendar: bool,
}

impl Default for Apps {
    fn default() -> Self {
        Self { clock: true, usage: true, timer: true, note: true, music: true, system: true, clipboard: true, shelf: true, net: true, links: true, focus: false, lyrics: true, calendar: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Link {
    pub name: String,
    /// a web address or the path of a program
    pub target: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Focus {
    /// only while the focus timer runs (otherwise always)
    pub only_timer: bool,
    /// words in a window's title or program name that count as a distraction
    pub words: Vec<String>,
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            only_timer: true,
            words: ["YouTube", "Instagram", "TikTok", "Twitter", "X.com", "Facebook", "Netflix", "Reddit", "Twitch"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub name: String,
    pub claude: bool,
    pub notify: bool,
    pub notify_question: bool,
    pub sound: bool,
    pub auto_expand: bool,
    pub hour12: bool,
    pub autostart: bool,
    pub hide_fullscreen: bool,
    /// ask Anthropic for the 5-hour / weekly use with Claude Code's saved login (off unless switched on)
    pub usage_from_account: bool,
    /// "hover" (opens after resting the mouse on it a moment) or "click"
    pub open_mode: String,
    pub shortcut: String,
    pub colors: Colors,
    pub apps: Apps,
    /// up to 4 of your own sites or programs in the island
    pub links: Vec<Link>,
    pub focus: Focus,
    /// a calendar's secret iCal address (Google Calendar, Outlook…)
    pub calendar_url: String,
    /// where each kind of notice shows: "card" (default), "windows" or "off"
    pub notices: std::collections::BTreeMap<String, String>,
    /// "limit almost full" at this % of the 5-hour limit (0 = never)
    pub limit_warn: u32,
    /// "compact" | "normal" | "large"
    pub size: String,
    /// "attached" (like a notch) | "floating" (just below the top, like on an iPhone)
    pub style: String,
    /// "black" | "glass"
    pub material: String,
    /// 0–80: how much you see through the island
    pub transparency: u32,
    /// glass takes the color of the album art that is playing
    pub music_tint: bool,
    /// what the resting island shows left and right: none, date, clock, weather, battery, cpu, ram, net, usage
    pub rest_left: String,
    pub rest_right: String,
    pub rest_clock: bool,
    /// "auto" | "12" | "24"
    pub clock_mode: String,
    pub volume_bar: bool,
    /// volume of the island's own sounds, 0–100
    pub sound_volume: u32,
    /// "classic" | "8bit" | "soft" | "bells"
    pub sound_pack: String,
    /// download and install new versions by itself (when no AI is busy)
    pub auto_update: bool,
    pub weather_city: String,
}

impl Default for Config {
    fn default() -> Self {
        let name = std::env::var("USERNAME").unwrap_or_default();
        let mut chars = name.chars();
        let name = match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        };
        Self {
            name,
            claude: false,
            notify: true,
            notify_question: true,
            sound: true,
            auto_expand: true,
            hour12: false,
            autostart: false,
            hide_fullscreen: true,
            usage_from_account: false,
            open_mode: "hover".into(),
            shortcut: "Ctrl+Alt+I".into(),
            colors: Colors::default(),
            apps: Apps::default(),
            links: Vec::new(),
            focus: Focus::default(),
            calendar_url: String::new(),
            notices: Default::default(),
            limit_warn: 80,
            size: "normal".into(),
            style: "attached".into(),
            material: "black".into(),
            transparency: 0,
            music_tint: true,
            rest_left: "date".into(),
            rest_right: "usage".into(),
            rest_clock: true,
            clock_mode: "auto".into(),
            volume_bar: true,
            sound_volume: 70,
            sound_pack: "classic".into(),
            auto_update: true,
            weather_city: String::new(),
        }
    }
}

impl Config {
    /// Returns the config and whether it was created now (first run).
    pub fn load(path: &Path) -> (Self, bool) {
        match std::fs::read_to_string(path) {
            Ok(text) => (serde_json::from_str(&text).unwrap_or_default(), false),
            Err(_) => (Self::default(), true),
        }
    }

    pub fn save(&self, path: &Path) {
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = crate::write_atomic(path, &text);
        }
    }
}
