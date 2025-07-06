use iced::Theme;

use serde::{ser::SerializeStruct, Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone)]
pub struct Config {
    pub theme: Theme,
}

impl Config {
    fn load_settings() -> Self {
        let settings_file_path = Self::settings_file_path();
        if let Err(_) = std::fs::exists(&settings_file_path) {
            let default_settings = Self {
                theme: Theme::default(),
            };
            let settings_json = serde_json::to_string(&default_settings).unwrap();
            std::fs::write(&settings_file_path, settings_json)
                .expect("Failed to write default settings");
            return default_settings;
        }

        if let Ok(settings_json) = std::fs::read_to_string(settings_file_path) {
            if let Ok(settings) = serde_json::from_str::<Self>(&settings_json) {
                return settings;
            } else {
                return Self {
                    theme: Theme::default(),
                };
            }
        } else {
            return Self {
                theme: Theme::default(),
            };
        }
    }

    pub fn update_settings(&self) {
        let settings_json = serde_json::to_string(self).expect("Failed to serialize settings");
        std::fs::write(Self::settings_file_path(), settings_json)
            .expect("Failed to write settings file");
    }

    fn settings_file_path() -> String {
        let settings_dir = home::home_dir()
            .map(|path| path.join(".ergon"))
            .unwrap_or_else(|| ".ergon".into());

        if !settings_dir.exists() {
            std::fs::create_dir_all(&settings_dir).expect("Failed to create settings directory");
        }

        settings_dir
            .join(SETTINGS_FILE)
            .to_string_lossy()
            .into_owned()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::load_settings()
    }
}

impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let theme_name = match self.theme {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            _ => "Default",
        };
        let mut state = serializer.serialize_struct("Config", 1)?;
        state.serialize_field("theme", theme_name)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Fields {
            Theme,
        }

        impl<'de> Deserialize<'de> for Fields {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldsVisitor;

                impl<'de> serde::de::Visitor<'de> for FieldsVisitor {
                    type Value = Fields;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("a field name")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "theme" => Ok(Fields::Theme),
                            _ => Err(E::unknown_field(value, &["theme"])),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldsVisitor)
            }
        }

        struct ConfigVisitor;
        impl<'de> serde::de::Visitor<'de> for ConfigVisitor {
            type Value = Config;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a configuration object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut theme = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Fields::Theme => {
                            if theme.is_some() {
                                return Err(serde::de::Error::duplicate_field("theme"));
                            }
                            let theme_name: &str = map.next_value()?;
                            theme = Some(match theme_name {
                                "Light" => Theme::Light,
                                "Dark" => Theme::Dark,
                                _ => Theme::default(),
                            });
                        }
                    }
                }

                let theme = theme.ok_or_else(|| serde::de::Error::missing_field("theme"))?;
                Ok(Config { theme })
            }
        }

        deserializer.deserialize_struct("Config", &["theme"], ConfigVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, Theme::default());
    }

    #[test]
    fn test_serialize_config() {
        let config = Config { theme: Theme::Dark };
        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains("\"theme\":\"Dark\""));
    }

    #[test]
    fn test_deserialize_config() {
        let json = r#"{"theme":"Light"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Light);
    }
}
