//! Localization (i18n): locale bundles with fallback and placeholder
//! interpolation, e.g. `translate("messages.welcome", &[("name", "Ada")])`.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// A set of locale bundles. Keys are flattened with dots
/// (`{"messages": {"welcome": "Hi"}}` becomes `messages.welcome`).
#[derive(Debug)]
pub struct Translator {
    bundles: RwLock<HashMap<String, HashMap<String, String>>>,
    fallback: String,
    current: RwLock<String>,
}

impl Translator {
    /// Create a translator whose fallback (and initial) locale is `fallback`.
    pub fn new(fallback: impl Into<String>) -> Self {
        let fallback = fallback.into();
        Self {
            bundles: RwLock::new(HashMap::new()),
            current: RwLock::new(fallback.clone()),
            fallback,
        }
    }

    /// Register a locale from nested JSON (or a flat key/value object).
    pub fn add_locale(&self, locale: impl Into<String>, messages: serde_json::Value) {
        let mut flat = HashMap::new();
        flatten(&messages, String::new(), &mut flat);
        if let Ok(mut bundles) = self.bundles.write() {
            bundles.insert(locale.into(), flat);
        }
    }

    /// Register a locale from flat `key -> text` pairs.
    pub fn add_messages(&self, locale: impl Into<String>, messages: HashMap<String, String>) {
        if let Ok(mut bundles) = self.bundles.write() {
            bundles.insert(locale.into(), messages);
        }
    }

    pub fn set_locale(&self, locale: impl Into<String>) {
        if let Ok(mut current) = self.current.write() {
            *current = locale.into();
        }
    }

    pub fn locale(&self) -> String {
        self.current.read().map(|c| c.clone()).unwrap_or_else(|_| self.fallback.clone())
    }

    /// Translate `key` in the current locale, falling back to the fallback
    /// locale, then to the key itself. `:name`-style placeholders (and
    /// `{name}`) are replaced from `replacements`.
    pub fn translate(&self, key: &str, replacements: &[(&str, &str)]) -> String {
        let current = self.locale();
        let bundles = match self.bundles.read() {
            Ok(bundles) => bundles,
            Err(_) => return key.to_string(),
        };
        let text = bundles
            .get(&current)
            .and_then(|bundle| bundle.get(key))
            .or_else(|| bundles.get(&self.fallback).and_then(|bundle| bundle.get(key)));
        let Some(text) = text else {
            return key.to_string();
        };
        let mut out = text.clone();
        for (name, value) in replacements {
            out = out
                .replace(&format!(":{name}"), value)
                .replace(&format!("{{{name}}}"), value);
        }
        out
    }

    /// Whether a translation exists for `key` in the current or fallback locale.
    pub fn has(&self, key: &str) -> bool {
        let bundles = match self.bundles.read() {
            Ok(bundles) => bundles,
            Err(_) => return false,
        };
        let current = self.locale();
        bundles
            .get(&current)
            .is_some_and(|bundle| bundle.contains_key(key))
            || bundles
                .get(&self.fallback)
                .is_some_and(|bundle| bundle.contains_key(key))
    }

    pub fn global() -> &'static Self {
        static TRANSLATOR: OnceLock<Translator> = OnceLock::new();
        TRANSLATOR.get_or_init(|| Translator::new("en"))
    }
}

fn flatten(value: &serde_json::Value, prefix: String, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                let next = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(inner, next, out);
            }
        }
        serde_json::Value::String(text) => {
            out.insert(prefix, text.clone());
        }
        _ => {}
    }
}

/// Translate using the process-global translator.
pub fn translate(key: &str, replacements: &[(&str, &str)]) -> String {
    Translator::global().translate(key, replacements)
}

/// Register a locale on the process-global translator.
pub fn add_locale(locale: impl Into<String>, messages: serde_json::Value) {
    Translator::global().add_locale(locale, messages);
}

/// Set the locale of the process-global translator.
pub fn set_locale(locale: impl Into<String>) {
    Translator::global().set_locale(locale);
}
