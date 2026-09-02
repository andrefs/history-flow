use crate::config::{AttributionMode, Config, ImportMode, MatchMode};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WebForm {
    pub target: Option<String>,
    pub attr_mode: Option<String>,
    pub mode: Option<String>,
    pub last: Option<usize>,
    pub nth: Option<usize>,
    pub match_mode: Option<String>,
    pub fuzzy_thresh: Option<f64>,
    pub probe: Option<String>, // "1" for size-up
}

impl WebForm {
    pub fn into_config(self) -> Config {
        let mut c = Config::default();
        if let Some(t) = self.target {
            c.import.url = Some(t);
        }
        if let Some(v) = self.attr_mode {
            c.attribution.mode = match v.as_str() {
                "last_editor" => AttributionMode::LastEditor,
                _ => AttributionMode::Provenance,
            };
        }
        if let Some(v) = self.mode {
            c.import.mode = match v.as_str() {
                "last" => ImportMode::Last,
                "nth" => ImportMode::Nth,
                _ => ImportMode::All,
            };
        }
        if let Some(v) = self.last {
            c.import.last = v;
        }
        if let Some(v) = self.nth {
            c.import.nth = v;
        }
        if let Some(v) = self.match_mode {
            c.attribution.match_mode = match v.as_str() {
                "fuzzy" => MatchMode::Fuzzy,
                _ => MatchMode::Exact,
            };
        }
        if let Some(v) = self.fuzzy_thresh {
            c.attribution.fuzzy_thresh = v;
        }
        c
    }
    pub fn is_probe(&self) -> bool {
        self.probe.as_deref() == Some("1")
    }
}
