use crate::ProviderError;
use openusage_plugin_engine::runtime::{MetricLine, PluginOutput, ProgressFormat};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
struct JsonOutput<'a> {
    providers: Vec<JsonProvider>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    errors: HashMap<&'a str, &'a ProviderError>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProvider {
    provider_id: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<String>,
    lines: Vec<JsonLine>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum JsonLine {
    Text {
        label: String,
        value: String,
    },
    Progress {
        label: String,
        used: f64,
        limit: f64,
        unit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        suffix: Option<String>,
        #[serde(rename = "resetsAt", skip_serializing_if = "Option::is_none")]
        resets_at: Option<String>,
        #[serde(rename = "periodDurationMs", skip_serializing_if = "Option::is_none")]
        period_duration_ms: Option<u64>,
    },
    Badge {
        label: String,
        text: String,
    },
}

fn convert_line(line: &MetricLine) -> JsonLine {
    match line {
        MetricLine::Text { label, value, .. } => JsonLine::Text {
            label: label.clone(),
            value: value.clone(),
        },
        MetricLine::Progress {
            label,
            used,
            limit,
            format,
            resets_at,
            period_duration_ms,
            ..
        } => {
            let (unit, suffix) = match format {
                ProgressFormat::Percent => ("percent".to_string(), None),
                ProgressFormat::Dollars => ("dollars".to_string(), None),
                ProgressFormat::Count { suffix } => ("count".to_string(), Some(suffix.clone())),
            };
            JsonLine::Progress {
                label: label.clone(),
                used: *used,
                limit: *limit,
                unit,
                suffix,
                resets_at: resets_at.clone(),
                period_duration_ms: *period_duration_ms,
            }
        }
        MetricLine::Badge { label, text, .. } => JsonLine::Badge {
            label: label.clone(),
            text: text.clone(),
        },
    }
}

pub fn format(outputs: &[PluginOutput], errors: &HashMap<String, ProviderError>) -> String {
    let providers: Vec<JsonProvider> = outputs
        .iter()
        .map(|o| JsonProvider {
            provider_id: o.provider_id.clone(),
            display_name: o.display_name.clone(),
            plan: o.plan.clone(),
            lines: o.lines.iter().map(convert_line).collect(),
        })
        .collect();

    let json_errors: HashMap<&str, &ProviderError> = errors
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let wrapper = JsonOutput {
        providers,
        errors: json_errors,
    };
    serde_json::to_string_pretty(&wrapper).unwrap_or_else(|e| {
        format!("{{\"error\": \"{}\"}}", e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openusage_plugin_engine::runtime::{MetricLine, PluginOutput, ProgressFormat};

    fn sample_output() -> PluginOutput {
        PluginOutput {
            provider_id: "claude".to_string(),
            display_name: "Claude".to_string(),
            plan: Some("Max (5x)".to_string()),
            lines: vec![
                MetricLine::Progress {
                    label: "Session".to_string(),
                    used: 80.0,
                    limit: 100.0,
                    format: ProgressFormat::Percent,
                    resets_at: None,
                    period_duration_ms: None,
                    color: None,
                },
                MetricLine::Text {
                    label: "Today".to_string(),
                    value: "1.5M tokens".to_string(),
                    color: None,
                    subtitle: None,
                },
            ],
            icon_url: "data:image/svg+xml;base64,AAAA".to_string(),
        }
    }

    #[test]
    fn json_output_contains_providers_key() {
        let output = format(&[sample_output()], &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("providers").is_some());
    }

    #[test]
    fn json_output_strips_icon_url() {
        let output = format(&[sample_output()], &HashMap::new());
        assert!(!output.contains("iconUrl"), "icon_url should be stripped from JSON output");
        assert!(!output.contains("icon_url"), "icon_url should be stripped from JSON output");
        assert!(!output.contains("AAAA"), "base64 icon data should not appear");
    }

    #[test]
    fn json_output_includes_provider_fields() {
        let output = format(&[sample_output()], &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let provider = &parsed["providers"][0];
        assert_eq!(provider["providerId"], "claude");
        assert_eq!(provider["displayName"], "Claude");
        assert_eq!(provider["plan"], "Max (5x)");
    }

    #[test]
    fn json_output_includes_lines() {
        let output = format(&[sample_output()], &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let lines = parsed["providers"][0]["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 2);

        // Progress line: flattened shape
        assert_eq!(lines[0]["type"], "progress");
        assert_eq!(lines[0]["label"], "Session");
        assert_eq!(lines[0]["used"], 80.0);
        assert_eq!(lines[0]["limit"], 100.0);
        assert_eq!(lines[0]["unit"], "percent");
        assert!(lines[0].get("format").is_none(), "format should be flattened, not nested");

        // Text line
        assert_eq!(lines[1]["type"], "text");
        assert_eq!(lines[1]["label"], "Today");
        assert_eq!(lines[1]["value"], "1.5M tokens");
    }

    #[test]
    fn json_output_skips_null_plan() {
        let mut o = sample_output();
        o.plan = None;
        let output = format(&[o], &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["providers"][0].get("plan").is_none());
    }

    #[test]
    fn json_output_empty_providers() {
        let output = format(&[], &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["providers"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_output_no_errors_key_when_empty() {
        let output = format(&[sample_output()], &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("errors").is_none(), "errors key should be absent when no errors");
    }

    #[test]
    fn json_output_includes_errors_key_when_present() {
        let mut errors = HashMap::new();
        errors.insert(
            "claude".to_string(),
            ProviderError {
                code: "provider_not_found".to_string(),
                message: "no plugin matches provider 'claude'".to_string(),
            },
        );
        let output = format(&[], &errors);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let errs = parsed.get("errors").expect("errors key should be present");
        let claude_err = errs.get("claude").expect("should have claude error");
        assert_eq!(claude_err["code"], "provider_not_found");
        assert_eq!(claude_err["message"], "no plugin matches provider 'claude'");
    }

    #[test]
    fn json_output_strips_color_and_subtitle_from_all_line_types() {
        let outputs = vec![PluginOutput {
            provider_id: "test".to_string(),
            display_name: "Test".to_string(),
            plan: None,
            lines: vec![
                MetricLine::Text {
                    label: "L".to_string(),
                    value: "V".to_string(),
                    color: Some("#ff0000".to_string()),
                    subtitle: Some("sub".to_string()),
                },
                MetricLine::Progress {
                    label: "P".to_string(),
                    used: 1.0,
                    limit: 100.0,
                    format: ProgressFormat::Percent,
                    resets_at: None,
                    period_duration_ms: None,
                    color: Some("#00ff00".to_string()),
                },
                MetricLine::Badge {
                    label: "B".to_string(),
                    text: "T".to_string(),
                    color: Some("#0000ff".to_string()),
                    subtitle: Some("badge-sub".to_string()),
                },
            ],
            icon_url: String::new(),
        }];
        let output = format(&outputs, &HashMap::new());
        assert!(!output.contains("color"), "color should not appear in JSON output");
        assert!(!output.contains("subtitle"), "subtitle should not appear in JSON output");
    }

    #[test]
    fn json_output_progress_count_emits_unit_and_suffix() {
        let outputs = vec![PluginOutput {
            provider_id: "test".to_string(),
            display_name: "Test".to_string(),
            plan: None,
            lines: vec![MetricLine::Progress {
                label: "Tokens".to_string(),
                used: 500.0,
                limit: 1000.0,
                format: ProgressFormat::Count {
                    suffix: "tokens".to_string(),
                },
                resets_at: None,
                period_duration_ms: None,
                color: None,
            }],
            icon_url: String::new(),
        }];
        let output = format(&outputs, &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let line = &parsed["providers"][0]["lines"][0];
        assert_eq!(line["unit"], "count");
        assert_eq!(line["suffix"], "tokens");
    }

    #[test]
    fn json_output_progress_dollars_emits_unit_dollars() {
        let outputs = vec![PluginOutput {
            provider_id: "test".to_string(),
            display_name: "Test".to_string(),
            plan: None,
            lines: vec![MetricLine::Progress {
                label: "Cost".to_string(),
                used: 5.0,
                limit: 100.0,
                format: ProgressFormat::Dollars,
                resets_at: None,
                period_duration_ms: None,
                color: None,
            }],
            icon_url: String::new(),
        }];
        let output = format(&outputs, &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let line = &parsed["providers"][0]["lines"][0];
        assert_eq!(line["unit"], "dollars");
        assert!(line.get("suffix").is_none(), "dollars should not have suffix");
    }

    #[test]
    fn json_output_progress_resets_at_and_period_duration_ms_present() {
        let outputs = vec![PluginOutput {
            provider_id: "test".to_string(),
            display_name: "Test".to_string(),
            plan: None,
            lines: vec![MetricLine::Progress {
                label: "Session".to_string(),
                used: 80.0,
                limit: 100.0,
                format: ProgressFormat::Percent,
                resets_at: Some("2025-01-01T00:00:00Z".to_string()),
                period_duration_ms: Some(18000000),
                color: None,
            }],
            icon_url: String::new(),
        }];
        let output = format(&outputs, &HashMap::new());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let line = &parsed["providers"][0]["lines"][0];
        assert_eq!(line["resetsAt"], "2025-01-01T00:00:00Z");
        assert_eq!(line["periodDurationMs"], 18000000);
    }
}
