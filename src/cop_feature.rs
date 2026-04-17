//! COP Feature — omegon Feature implementation for COP display surface tools.
//!
//! Registers `cop_write`, `cop_clear`, and `cop_layout` as native tools in
//! omegon's EventBus.  Tool execution returns immediate success — the actual
//! COP state update happens in auspex's event interceptor when it sees the
//! ToolStart event args flow through.

use async_trait::async_trait;
use omegon_traits::{ContentBlock, Feature, ToolDefinition, ToolResult};
use serde_json::Value;

pub struct CopFeature;

#[async_trait]
impl Feature for CopFeature {
    fn name(&self) -> &str {
        "cop"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "cop_write".into(),
                label: "COP Write".into(),
                description: "Write structured content to a named region of the Common Operating Picture. \
                    Content types: table, status_card, alert_feed, kv_grid, text_block, code_block, metric. \
                    Regions: center (dominant), north, south, east, west (quadrants).".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "region": {
                            "type": "string",
                            "description": "Target region: center, north, south, east, west",
                            "default": "center"
                        },
                        "content_type": {
                            "type": "string",
                            "enum": ["table", "status_card", "alert_feed", "kv_grid", "text_block", "code_block", "metric"],
                            "description": "The type of structured content to render"
                        },
                        "title": {
                            "type": "string",
                            "description": "Optional title displayed above the region content"
                        },
                        "data": {
                            "type": "object",
                            "description": "Content payload — schema depends on content_type. \
                                table: {columns: [str], rows: [[val]]}. \
                                status_card: {label, status, detail?, severity?}. \
                                alert_feed: {items: [{message, severity?, source?, timestamp?}]}. \
                                kv_grid: {pairs: [{key, value}]} or flat {key: value} object. \
                                text_block: {text}. \
                                code_block: {code, language?}. \
                                metric: {label, value, unit?, trend?}."
                        }
                    },
                    "required": ["content_type", "data"]
                }),
            },
            ToolDefinition {
                name: "cop_clear".into(),
                label: "COP Clear".into(),
                description: "Clear a named COP region, or clear all regions if no region specified.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "region": {
                            "type": "string",
                            "description": "Region to clear. Omit to clear all regions."
                        }
                    }
                }),
            },
            ToolDefinition {
                name: "cop_layout".into(),
                label: "COP Layout".into(),
                description: "Configure which COP regions are active. Default segmenta: center with north/south/east/west quadrants.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "regions": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Ordered list of region names to activate"
                        }
                    },
                    "required": ["regions"]
                }),
            },
        ]
    }

    async fn execute(
        &self,
        tool_name: &str,
        _call_id: &str,
        _args: Value,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        // The real COP state update happens in auspex's event interceptor
        // (controller.try_intercept_cop_tool_event) when it sees the ToolStart
        // event.  This execute() just tells omegon the tool succeeded.
        Ok(ToolResult {
            content: vec![ContentBlock::Text {
                text: format!("{tool_name}: ok"),
            }],
            details: Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cop_feature_exposes_three_tools() {
        let feature = CopFeature;
        let tools = feature.tools();
        assert_eq!(tools.len(), 3);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"cop_write"));
        assert!(names.contains(&"cop_clear"));
        assert!(names.contains(&"cop_layout"));
    }

    #[test]
    fn cop_feature_name() {
        assert_eq!(CopFeature.name(), "cop");
    }

    #[tokio::test]
    async fn cop_feature_execute_returns_success() {
        let feature = CopFeature;
        let cancel = tokio_util::sync::CancellationToken::new();
        let result = feature
            .execute("cop_write", "call-1", serde_json::json!({}), cancel)
            .await
            .expect("execute should succeed");

        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            ContentBlock::Text { text } => assert!(text.contains("ok")),
            _ => panic!("expected text content block"),
        }
    }
}
