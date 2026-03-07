//! Spec loading, recursive override merging, and config-diff generation.

use crate::model::{CompareSpec, ConfigDiff, ResolvedSpec, VariantRunContext};
use anyhow::{anyhow, Context, Result};
use redis_web_core::config::Config;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_spec(spec_path: &Path, workspace_root: &Path) -> Result<ResolvedSpec> {
    let spec_abspath = if spec_path.is_absolute() {
        spec_path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(spec_path)
    };
    let spec_dir = spec_abspath
        .parent()
        .ok_or_else(|| anyhow!("spec path has no parent: {}", spec_abspath.display()))?;
    let bytes = fs::read(&spec_abspath)
        .with_context(|| format!("failed to read spec {}", spec_abspath.display()))?;
    let spec: CompareSpec = match spec_abspath.extension().and_then(|ext| ext.to_str()) {
        Some("json") => serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse JSON spec {}", spec_abspath.display()))?,
        Some("yaml") | Some("yml") => serde_yaml::from_slice(&bytes)
            .with_context(|| format!("failed to parse YAML spec {}", spec_abspath.display()))?,
        _ => serde_yaml::from_slice(&bytes)
            .or_else(|_| serde_json::from_slice(&bytes))
            .with_context(|| {
                format!(
                    "failed to parse YAML or JSON spec {}",
                    spec_abspath.display()
                )
            })?,
    };

    let output_root = spec
        .output_dir
        .map(|path| resolve_relative(spec_dir, path))
        .unwrap_or_else(|| workspace_root.join("target/perf"));

    Ok(ResolvedSpec {
        base_config: resolve_relative(spec_dir, spec.base_config),
        output_root,
        variants: spec.variants,
    })
}

pub(crate) fn load_json_value(path: &Path) -> Result<Value> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse JSON config {}", path.display()))
}

pub(crate) fn build_variant_context(
    name: &str,
    base_raw: &Value,
    overrides: &Value,
    baseline_value: Option<&Value>,
) -> Result<(VariantRunContext, Value)> {
    let mut merged = base_raw.clone();
    merge_json(&mut merged, overrides)?;
    let config = Config::from_value(merged)
        .with_context(|| format!("invalid config for variant `{name}`"))?;
    let effective_value =
        serde_json::to_value(&config).context("failed to serialize effective config")?;
    let diff = baseline_value
        .map(|baseline| flatten_diff(baseline, &effective_value))
        .unwrap_or_default();

    Ok((
        VariantRunContext {
            name: name.to_string(),
            config,
            diff,
        },
        effective_value,
    ))
}

fn resolve_relative(base_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

pub(crate) fn merge_json(base: &mut Value, overrides: &Value) -> Result<()> {
    match (base, overrides) {
        (_, Value::Null) => Ok(()),
        (Value::Object(base_map), Value::Object(override_map)) => {
            for (key, override_value) in override_map {
                match base_map.get_mut(key) {
                    Some(base_value) => merge_json(base_value, override_value)?,
                    None => {
                        base_map.insert(key.clone(), override_value.clone());
                    }
                }
            }
            Ok(())
        }
        (base_slot, value) => {
            *base_slot = value.clone();
            Ok(())
        }
    }
}

pub(crate) fn flatten_diff(base: &Value, variant: &Value) -> Vec<ConfigDiff> {
    let mut diffs = Vec::new();
    collect_diff_entries(String::new(), base, variant, &mut diffs);
    diffs
}

fn collect_diff_entries(path: String, base: &Value, variant: &Value, diffs: &mut Vec<ConfigDiff>) {
    if base == variant {
        return;
    }

    match (base, variant) {
        (Value::Object(base_map), Value::Object(variant_map)) => {
            let keys: BTreeSet<_> = base_map.keys().chain(variant_map.keys()).cloned().collect();
            for key in keys {
                let next_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match (base_map.get(&key), variant_map.get(&key)) {
                    (Some(base_value), Some(variant_value)) => {
                        collect_diff_entries(next_path, base_value, variant_value, diffs);
                    }
                    (Some(base_value), None) => diffs.push(ConfigDiff {
                        key: next_path,
                        before: display_json_value(base_value),
                        after: "<unset>".to_string(),
                    }),
                    (None, Some(variant_value)) => diffs.push(ConfigDiff {
                        key: next_path,
                        before: "<unset>".to_string(),
                        after: display_json_value(variant_value),
                    }),
                    (None, None) => {}
                }
            }
        }
        _ => diffs.push(ConfigDiff {
            key: path,
            before: display_json_value(base),
            after: display_json_value(variant),
        }),
    }
}

fn display_json_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "<unprintable>".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_json_recursively_replaces_leaf_values() {
        let mut base = json!({
            "transport_mode": "rest",
            "grpc": {"port": 7379, "enable_reflection": false},
            "websockets": false
        });
        let overrides = json!({
            "grpc": {"enable_reflection": true},
            "websockets": true
        });

        merge_json(&mut base, &overrides).unwrap();

        assert_eq!(base["grpc"]["port"], json!(7379));
        assert_eq!(base["grpc"]["enable_reflection"], json!(true));
        assert_eq!(base["websockets"], json!(true));
    }

    #[test]
    fn flatten_diff_reports_nested_keys() {
        let baseline = json!({
            "transport_mode": "rest",
            "grpc": {"enable_reflection": false},
            "websockets": false
        });
        let variant = json!({
            "transport_mode": "grpc",
            "grpc": {"enable_reflection": true},
            "websockets": false
        });

        let diffs = flatten_diff(&baseline, &variant);
        assert!(diffs
            .iter()
            .any(|diff| diff.key == "transport_mode" && diff.after == "grpc"));
        assert!(diffs
            .iter()
            .any(|diff| diff.key == "grpc.enable_reflection" && diff.after == "true"));
    }
}
