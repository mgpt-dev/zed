use gpui::{actions, Action};
use schemars::JsonSchema;
use serde::Deserialize;

actions!(
    planning_panel,
    [
        ToggleFocus,
        NewPlan,
        SavePlan,
        LoadPlan,
        ExportTasks,
        AddNode,
        DeleteNode,
        UpdateNode,
        RequestAISuggestion,
        ApplySuggestion,
        RejectSuggestion,
    ]
);

/// Triggers inline AI assistance for the selected plan nodes in the Planning Panel.
/// This action shows an inline prompt editor at the selected text/nodes,
/// allowing the user to request AI suggestions for refining, expanding, or
/// critiquing specific parts of their plan.
#[derive(Clone, Default, Deserialize, PartialEq, JsonSchema, Action)]
#[action(namespace = planning_panel)]
#[serde(deny_unknown_fields)]
pub struct InlineAssist {
    /// Optional initial prompt to pre-fill in the inline assist editor
    pub prompt: Option<String>,
}

