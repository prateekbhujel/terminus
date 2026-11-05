//! Advanced Table & API Filtering Engine
//!
//! Provides date range pickers, numeric range sliders, multi-select tags,
//! boolean toggles, fuzzy search, and saved filter presets.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of table and query filter
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterType {
    /// Date range with start and end ISO dates
    DateRange,
    /// Numeric range slider with min, max, and step
    RangeSlider { min: f64, max: f64, step: f64 },
    /// Multi-select badges / tag filters
    MultiSelect { options: Vec<(String, String)> },
    /// Simple boolean switch / toggle
    Boolean,
    /// Fuzzy substring full-text search
    FuzzySearch,
}

/// A filter definition attached to a resource table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub key: String,
    pub label: String,
    pub filter_type: FilterType,
    pub default_value: Option<String>,
}

impl Filter {
    /// Date range filter (e.g., "created_at" between date A and date B)
    pub fn date_range(key: impl Into<String>) -> Self {
        let key_str = key.into();
        let label = crate::resource::headline(&key_str);
        Self {
            key: key_str,
            label,
            filter_type: FilterType::DateRange,
            default_value: None,
        }
    }

    /// Numeric range slider filter (e.g., price between 10 and 1000)
    pub fn range_slider(key: impl Into<String>, min: f64, max: f64, step: f64) -> Self {
        let key_str = key.into();
        let label = crate::resource::headline(&key_str);
        Self {
            key: key_str,
            label,
            filter_type: FilterType::RangeSlider { min, max, step },
            default_value: None,
        }
    }

    /// Multi-select tag/badge filter
    pub fn multi_select(key: impl Into<String>, options: Vec<(&str, &str)>) -> Self {
        let key_str = key.into();
        let label = crate::resource::headline(&key_str);
        Self {
            key: key_str,
            label,
            filter_type: FilterType::MultiSelect {
                options: options
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            },
            default_value: None,
        }
    }

    /// Boolean toggle filter
    pub fn boolean(key: impl Into<String>) -> Self {
        let key_str = key.into();
        let label = crate::resource::headline(&key_str);
        Self {
            key: key_str,
            label,
            filter_type: FilterType::Boolean,
            default_value: None,
        }
    }

    /// Fuzzy search filter
    pub fn fuzzy(key: impl Into<String>) -> Self {
        let key_str = key.into();
        let label = crate::resource::headline(&key_str);
        Self {
            key: key_str,
            label,
            filter_type: FilterType::FuzzySearch,
            default_value: None,
        }
    }

    /// Set a custom label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set default filter value
    pub fn default_value(mut self, val: impl Into<String>) -> Self {
        self.default_value = Some(val.into());
        self
    }
}

/// A saved filter preset (e.g., "Overdue Orders", "VIP Customers")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterPreset {
    pub name: String,
    pub label: String,
    pub criteria: HashMap<String, String>,
}

impl FilterPreset {
    pub fn make(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            criteria: HashMap::new(),
        }
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.criteria.insert(key.into(), value.into());
        self
    }
}

/// Filter Evaluation Engine: checks whether a record matches active filter parameters
pub fn matches_filters(
    values: &HashMap<String, String>,
    filters: &HashMap<String, String>,
) -> bool {
    for (key, filter_val) in filters {
        if filter_val.trim().is_empty() {
            continue;
        }

        if key.ends_with("_from") {
            let base_key = key.trim_end_matches("_from");
            if let Some(record_val) = values.get(base_key) {
                if record_val < filter_val {
                    return false;
                }
            }
        } else if key.ends_with("_to") {
            let base_key = key.trim_end_matches("_to");
            if let Some(record_val) = values.get(base_key) {
                if record_val > filter_val {
                    return false;
                }
            }
        } else if key.ends_with("_min") {
            let base_key = key.trim_end_matches("_min");
            if let Some(record_val) = values.get(base_key) {
                let rec_num: f64 = record_val.parse().unwrap_or(0.0);
                let filter_num: f64 = filter_val.parse().unwrap_or(0.0);
                if rec_num < filter_num {
                    return false;
                }
            }
        } else if key.ends_with("_max") {
            let base_key = key.trim_end_matches("_max");
            if let Some(record_val) = values.get(base_key) {
                let rec_num: f64 = record_val.parse().unwrap_or(0.0);
                let filter_num: f64 = filter_val.parse().unwrap_or(0.0);
                if rec_num > filter_num {
                    return false;
                }
            }
        } else if let Some(record_val) = values.get(key) {
            // Check comma-separated multi-select or direct match
            if filter_val.contains(',') {
                let selected: Vec<&str> = filter_val.split(',').map(|s| s.trim()).collect();
                if !selected.contains(&record_val.as_str()) {
                    return false;
                }
            } else if !record_val.to_lowercase().contains(&filter_val.to_lowercase()) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}
