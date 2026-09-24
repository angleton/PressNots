use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct AliasStore {
    path: PathBuf,
    aliases: HashMap<String, String>,
}

impl AliasStore {
    pub(crate) fn load_default() -> Self {
        let path = default_path();
        let aliases = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| parse(&contents).ok())
            .unwrap_or_default();
        Self { path, aliases }
    }

    pub(crate) fn get(&self, device_key: &str) -> Option<&str> {
        self.aliases.get(device_key).map(String::as_str)
    }

    pub(crate) fn rename(&mut self, device_key: &str, alias: &str) -> io::Result<()> {
        let alias = alias.trim();
        if alias.is_empty() {
            self.aliases.remove(device_key);
        } else {
            self.aliases.insert(device_key.to_owned(), alias.to_owned());
        }
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serialize(&self.aliases).map_err(io::Error::other)?;
        fs::write(&self.path, json)
    }
}

fn default_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("PressNots")
        .join("device_aliases.json")
}

fn parse(contents: &str) -> serde_json::Result<HashMap<String, String>> {
    serde_json::from_str(contents)
}

fn serialize(aliases: &HashMap<String, String>) -> serde_json::Result<String> {
    serde_json::to_string_pretty(aliases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn json_round_trip_preserves_device_paths_and_unicode_aliases() {
        let aliases = HashMap::from([(
            r"\\?\HID#VID_1234&PID_5678".to_owned(),
            "Editing Keyboard".to_owned(),
        )]);

        assert_eq!(parse(&serialize(&aliases).unwrap()).unwrap(), aliases);
    }

    #[test]
    fn malformed_json_is_rejected() {
        assert!(parse("not json").is_err());
    }

    #[test]
    fn reset_and_undo_are_persisted() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("pressnots-test-{unique}"));
        let path = directory.join("device_aliases.json");
        let mut store = AliasStore {
            path: path.clone(),
            aliases: HashMap::new(),
        };

        store.rename("device-key", "Studio Keyboard").unwrap();
        store.rename("device-key", "").unwrap();
        assert!(
            !parse(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .contains_key("device-key")
        );

        store.rename("device-key", "Studio Keyboard").unwrap();
        assert_eq!(
            parse(&fs::read_to_string(&path).unwrap()).unwrap()["device-key"],
            "Studio Keyboard"
        );

        fs::remove_dir_all(directory).unwrap();
    }
}
