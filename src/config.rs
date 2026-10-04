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
    ///
    /// none = match all
    #[serde(deserialize_with = "deserialize_regex", default)]
    pub repo_pattern: Option<Regex>,
    /// regex pattern for tag names
    ///
    /// none = match all
    #[serde(deserialize_with = "deserialize_regex", default)]
    pub tag_pattern: Option<Regex>,
    /// how many tags to keep
    ///
    /// none = no limit (no tag is treated as "overflow")
    #[serde(default)]
    pub max_tags: Option<usize>,
    /// for tags over `max_tags`, delete if age is over this
    ///
    /// none = delete all overflow
    #[serde(deserialize_with = "deserialize_duration", default)]
    pub overflow_max_age: Option<TimeDelta>,
    /// delete any tag older than this
    ///
    /// none = don't delete tags under `max_tags`
    #[serde(deserialize_with = "deserialize_duration", default)]
    pub max_age: Option<TimeDelta>,
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

impl Rule {
    #[must_use]
    pub fn is_match(&self, repo: &str, tag: &str) -> bool {
        self.repo_pattern.as_ref().is_none_or(|p| p.is_match(repo))
            && self.tag_pattern.as_ref().is_none_or(|p| p.is_match(tag))
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

fn deserialize_regex<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Regex>, D::Error> {
    struct RegexVisitor;
    impl Visitor<'_> for RegexVisitor {
        type Value = Option<Regex>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a valid regex pattern")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Regex::new(v).map_err(E::custom).map(Some)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_str(RegexVisitor)
}

fn deserialize_duration<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<TimeDelta>, D::Error> {
    struct TimeDeltaVisitor;
    impl Visitor<'_> for TimeDeltaVisitor {
        type Value = Option<TimeDelta>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number with a time unit suffix")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let number: i64 = v[0..v.len() - 1].trim().parse().map_err(E::custom)?;
            match &v[v.len() - 1..] {
                "s" => TimeDelta::try_seconds(number)
                    .ok_or_else(|| {
                        E::invalid_value(
                            Unexpected::Str(v),
                            &"a duration that doesn't exceed the limit",
                        )
                    })
                    .map(Some),
                "m" => TimeDelta::try_minutes(number)
                    .ok_or_else(|| {
                        E::invalid_value(
                            Unexpected::Str(v),
                            &"a duration that doesn't exceed the limit",
                        )
                    })
                    .map(Some),
                "h" => TimeDelta::try_hours(number)
                    .ok_or_else(|| {
                        E::invalid_value(
                            Unexpected::Str(v),
                            &"a duration that doesn't exceed the limit",
                        )
                    })
                    .map(Some),
                "d" => TimeDelta::try_days(number)
                    .ok_or_else(|| {
                        E::invalid_value(
                            Unexpected::Str(v),
                            &"a duration that doesn't exceed the limit",
                        )
                    })
                    .map(Some),
                "w" => TimeDelta::try_weeks(number)
                    .ok_or_else(|| {
                        E::invalid_value(
                            Unexpected::Str(v),
                            &"a duration that doesn't exceed the limit",
                        )
                    })
                    .map(Some),
                _ => Err(E::invalid_value(
                    Unexpected::Str(v),
                    &"a supported time unit suffix",
                )),
            }
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_str(TimeDeltaVisitor)
}
