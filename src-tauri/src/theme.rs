//! Reads the active Omarchy theme palette so the UI can follow theme changes.
use std::collections::HashMap;
use std::path::PathBuf;

fn colors_path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"))
        .join("omarchy/current/theme/colors.toml")
}

pub fn parse(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let v = v.trim().trim_matches('"');
            (!k.trim().starts_with('#') && !v.is_empty()).then(|| (k.trim().to_string(), v.to_string()))
        })
        .collect()
}

pub fn colors() -> Option<HashMap<String, String>> {
    std::fs::read_to_string(colors_path()).ok().map(|t| parse(&t))
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_toml_colors() {
        let m = super::parse("mode = \"dark\"\n# c\naccent = \"#faa968\"\n");
        assert_eq!(m["mode"], "dark");
        assert_eq!(m["accent"], "#faa968");
    }
}
