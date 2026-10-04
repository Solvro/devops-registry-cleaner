use std::{env, fmt, fs};

use chrono::TimeDelta;
use cloneable_errors::{ErrorContext, ResContext};
use regex::Regex;
use reqwest::Url;
use serde::{
    Deserialize, Deserializer,
    de::{Unexpected, Visitor},
};

use crate::api::client::Client;

#[derive(Deserialize)]
pub struct Config {
    pub registry: RegistryConfig,
    pub rules: Box<[Rule]>,
}

#[derive(Deserialize)]
pub struct RegistryConfig {
    #[serde(deserialize_with = "deserialize_url")]
    pub base_url: Url,
    pub username: Box<str>,
    pub password: Box<str>,
}

#[derive(Deserialize)]
pub struct Rule {
    /// regex pattern for repo names
    #[serde(deserialize_with = "deserialize_regex")]
    pub repo_pattern: Regex,
    /// regex pattern for tag names
    #[serde(deserialize_with = "deserialize_regex")]
    pub tag_pattern: Regex,
    /// how many tags to keep
    pub max_tags: usize,
    /// for tags over `max_tags`, delete if age is over this
    #[serde(deserialize_with = "deserialize_duration")]
    pub overflow_max_age: TimeDelta,
    /// delete any tag older than this
    #[serde(deserialize_with = "deserialize_duration")]
    pub max_age: TimeDelta,
}

impl Config {
    pub fn get() -> Result<Self, ErrorContext> {
        let config_path = env::var_os("CLEANER_CONFIG_FILE")
            .unwrap_or_else(|| "/etc/devops-registry-cleaner.toml".into());
        let file = fs::read(&config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

        toml::from_slice(&file).with_context(|| {
            format!(
                "Failed to parse the contents of {} as Config",
                config_path.display()
            )
        })
    }

    pub fn make_client(&self) -> Result<Client<'_>, ErrorContext> {
        Client::new(&self.registry)
    }
}

fn deserialize_url<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Url, D::Error> {
    struct UrlVisitor;
    impl Visitor<'_> for UrlVisitor {
        type Value = Url;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a valid base url")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let url = Url::parse(v).map_err(E::custom)?;
            if url.scheme() != "http" && url.scheme() != "https" {
                return Err(E::invalid_value(Unexpected::Str(v), &"a http(s) URL"));
            }
            if url.cannot_be_a_base() {
                return Err(E::invalid_value(Unexpected::Str(v), &"a valid base URL"));
            }
            Ok(url)
        }
    }

    deserializer.deserialize_str(UrlVisitor)
}

fn deserialize_regex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Regex, D::Error> {
    struct RegexVisitor;
    impl Visitor<'_> for RegexVisitor {
        type Value = Regex;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a valid regex pattern")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Regex::new(v).map_err(E::custom)
        }
    }

    deserializer.deserialize_str(RegexVisitor)
}

fn deserialize_duration<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TimeDelta, D::Error> {
    struct TimeDeltaVisitor;
    impl Visitor<'_> for TimeDeltaVisitor {
        type Value = TimeDelta;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number with a time unit suffix")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let number: i64 = v[0..v.len() - 1].trim().parse().map_err(E::custom)?;
            match &v[v.len() - 1..] {
                "s" => TimeDelta::try_seconds(number).ok_or_else(|| {
                    E::invalid_value(
                        Unexpected::Str(v),
                        &"a duration that doesn't exceed the limit",
                    )
                }),
                "m" => TimeDelta::try_minutes(number).ok_or_else(|| {
                    E::invalid_value(
                        Unexpected::Str(v),
                        &"a duration that doesn't exceed the limit",
                    )
                }),
                "h" => TimeDelta::try_hours(number).ok_or_else(|| {
                    E::invalid_value(
                        Unexpected::Str(v),
                        &"a duration that doesn't exceed the limit",
                    )
                }),
                "d" => TimeDelta::try_days(number).ok_or_else(|| {
                    E::invalid_value(
                        Unexpected::Str(v),
                        &"a duration that doesn't exceed the limit",
                    )
                }),
                "w" => TimeDelta::try_weeks(number).ok_or_else(|| {
                    E::invalid_value(
                        Unexpected::Str(v),
                        &"a duration that doesn't exceed the limit",
                    )
                }),
                _ => Err(E::invalid_value(
                    Unexpected::Str(v),
                    &"a supported time unit suffix",
                )),
            }
        }
    }

    deserializer.deserialize_str(TimeDeltaVisitor)
}
