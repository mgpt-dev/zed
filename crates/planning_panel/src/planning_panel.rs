use anyhow::Result;
use chrono::{DateTime, Utc};
use db::kvp::KEY_VALUE_STORE;
use editor::{CurrentLineHighlight, Editor, EditorEvent};
use futures::StreamExt;
use gpui::{
    App, AsyncWindowContext, ClipboardItem, Context, DismissEvent, Entity, EventEmitter,
    Focusable, FocusHandle, Render, SharedString, Subscription, Task, WeakEntity, Window, px,
};
use language_model::{
    LanguageModelRegistry, LanguageModelRequest, LanguageModelRequestMessage, Role,
};
use planning::{
    Plan, PlanEvent, PlanId, PlanMetadata, PlanNode, PlanVersion, PlanningState, NodeId, NodeType,
    TemplateRegistry, derive_tasks_from_plan, tasks_to_markdown,
    parse_markdown_to_plan, render_plan_to_markdown,
};
use serde::{Deserialize, Serialize};
use settings::SoftWrap;
use std::sync::Arc;
use ui::prelude::*;
use ui::{ListItem, Tooltip};
use workspace::{
    dock::{DockPosition, Panel, PanelEvent},
    Workspace,
};

const PLANNING_PANEL_KEY: &str = "PlanningPanel";
const PLANNING_PLANS_KEY: &str = "PlanningPanelPlans";

/// The current view state of the planning panel
#[derive(Debug, Clone, PartialEq)]
pub enum PlanningPanelView {
    /// Default: Show list of saved plans
    PlanList,
    /// User is creating a new plan with AI assistance
    NewPlanDialog,
    /// User is editing a plan in the markdown editor
    PlanEditor,
}

impl Default for PlanningPanelView {
    fn default() -> Self {
        Self::PlanList
    }
}

/// Summary of a saved plan for display in the list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPlanSummary {
    pub id: PlanId,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The full markdown content of the plan
    pub content: String,
}

/// Result of AI template inference
#[derive(Debug, Clone)]
pub struct TemplateInferenceResult {
    /// Selected template names
    pub templates: Vec<String>,
    /// Brief explanation of why these templates were selected
    pub explanation: String,
}

/// An AI suggestion for plan refinement
#[derive(Clone, Debug)]
pub struct AiSuggestion {
    pub id: usize,
    pub description: String,
    pub target_node: Option<NodeId>,
    pub suggestion_type: AiSuggestionType,
    pub content: String,
}

#[derive(Clone, Debug)]
pub enum AiSuggestionType {
    /// Add a new child node
    AddChild,
    /// Update node content
    UpdateContent,
    /// Add a sibling node
    AddSibling,
    /// General critique/feedback
    Critique,
}

pub struct PlanningPanel {
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    state: PlanningState,
    template_registry: Arc<TemplateRegistry>,
    width: Option<Pixels>,
    /// Current view state
    current_view: PlanningPanelView,
    /// List of saved plans
    saved_plans: Vec<SavedPlanSummary>,
    /// ID of the currently active plan (if editing)
    active_plan_id: Option<PlanId>,
    /// Editor for new plan description input
    plan_input_editor: Entity<Editor>,
    /// Subscription to plan input editor events
    _plan_input_subscription: Subscription,
    /// User's input description for new plan
    plan_input_description: String,
    /// Result of AI template inference (set after user submits description)
    template_inference_result: Option<TemplateInferenceResult>,
    /// Whether AI is currently inferring templates
    inferring_templates: bool,
    /// Whether AI is currently generating the plan
    generating_plan: bool,
    /// Show template selector (legacy, kept for compatibility)
    show_template_selector: bool,
    /// Markdown editor for plan content
    markdown_editor: Entity<Editor>,
    /// Subscription to editor events
    _editor_subscription: Subscription,
    /// Editor for plan title
    plan_title_editor: Entity<Editor>,
    /// Subscription to plan title editor events
    _plan_title_subscription: Subscription,
    /// AI suggestions pending approval
    ai_suggestions: Vec<AiSuggestion>,
    /// Next suggestion ID
    next_suggestion_id: usize,
    /// Whether AI is currently generating suggestions
    ai_loading: bool,
    /// Current AI generation task
    _ai_task: Option<Task<()>>,
    /// Whether the plan has unsaved changes
    plan_dirty: bool,
    /// Whether to suppress dirty marking (during programmatic editor updates)
    suppress_dirty: bool,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct SerializedPlanningPanel {
    width: Option<f32>,
    /// List of saved plans
    #[serde(default)]
    saved_plans: Vec<SavedPlanSummary>,
    /// ID of the currently active plan
    #[serde(default)]
    active_plan_id: Option<PlanId>,
}

impl PlanningPanel {
    fn serialization_key(workspace: &Workspace) -> Option<String> {
        workspace
            .database_id()
            .map(|id| i64::from(id).to_string())
            .or(workspace.session_id())
            .map(|id| format!("{}-{:?}", PLANNING_PANEL_KEY, id))
    }

    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        let serialized_panel = match workspace
            .read_with(&cx, |workspace, _| {
                PlanningPanel::serialization_key(workspace)
            })
            .ok()
            .flatten()
        {
            Some(serialization_key) => cx
                .background_spawn(async move { KEY_VALUE_STORE.read_kvp(&serialization_key) })
                .await
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str::<SerializedPlanningPanel>(&s).ok()),
            None => None,
        };

        workspace.update_in(&mut cx, |workspace, window, cx| {
            let panel = cx.new(|cx| Self::new(workspace, window, cx));

            if let Some(serialized_panel) = &serialized_panel {
                panel.update(cx, |panel, cx| {
                    panel.width = serialized_panel.width.map(|w| Pixels::from(w));
                    panel.saved_plans = serialized_panel.saved_plans.clone();
                    panel.active_plan_id = serialized_panel.active_plan_id;

                    // If there was an active plan, restore it
                    if let Some(plan_id) = panel.active_plan_id {
                        if let Some(saved_plan) = panel.saved_plans.iter().find(|p| p.id == plan_id) {
                            // Parse and load the plan
                            if let Some(mut plan) = parse_markdown_to_plan(&saved_plan.content) {
                                // IMPORTANT: Preserve the original plan ID from saved_plans
                                // parse_markdown_to_plan generates a new ID, but we need to keep
                                // the original so that save_current_plan can find the existing entry
                                plan.id = plan_id;

                                let plan_title = plan.metadata.title.clone();
                                let event = PlanEvent::PlanCreated {
                                    plan,
                                    timestamp: Utc::now(),
                                };
                                let _ = panel.state.apply_event(event);
                                panel.markdown_editor.update(cx, |editor, cx| {
                                    editor.set_text(saved_plan.content.clone(), window, cx);
                                });
                                // Also set the title editor
                                panel.plan_title_editor.update(cx, |editor, cx| {
                                    editor.set_text(plan_title, window, cx);
                                });
                                panel.current_view = PlanningPanelView::PlanEditor;
                            }
                        }
                    }
                    cx.notify();
                });
            }

            panel
        })
    }

    pub fn new(
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create multi-line markdown editor for plan content
        let markdown_editor = cx.new(|cx| Editor::multi_line(window, cx));
        markdown_editor.update(cx, |editor, cx| {
            editor.set_placeholder_text("Start planning...", window, cx);
            editor.set_soft_wrap_mode(SoftWrap::EditorWidth, cx);
        });

        // Create editor for plan input description (no line numbers, minimal UI)
        let plan_input_editor = cx.new(|cx| Editor::multi_line(window, cx));
        plan_input_editor.update(cx, |editor, cx| {
            editor.set_placeholder_text(
                "Describe what you want to plan (e.g., 'I need to refactor the auth system to use OAuth2')",
                window,
                cx,
            );
            editor.set_soft_wrap_mode(SoftWrap::EditorWidth, cx);
            editor.set_show_line_numbers(false, cx);
            editor.set_show_gutter(false, cx);
            editor.set_current_line_highlight(Some(CurrentLineHighlight::None));
        });

        // Create single-line editor for plan title
        let plan_title_editor = cx.new(|cx| Editor::single_line(window, cx));
        plan_title_editor.update(cx, |editor, cx| {
            editor.set_placeholder_text("Plan Title", window, cx);
        });

        // Subscribe to editor events to sync content with plan state
        let editor_subscription = cx.subscribe_in(
            &markdown_editor,
            window,
            |panel, _, event, _window, cx| {
                if let EditorEvent::BufferEdited { .. } = event {
                    panel.sync_editor_to_plan(cx);
                    // Only mark as dirty if not suppressed (i.e., user is editing, not programmatic)
                    if !panel.suppress_dirty {
                        panel.plan_dirty = true;
                    }
                }
            },
        );

        // Subscribe to plan input editor events
        let plan_input_subscription = cx.subscribe_in(
            &plan_input_editor,
            window,
            |panel, _, event, _window, cx| {
                if let EditorEvent::BufferEdited { .. } = event {
                    panel.plan_input_description = panel.plan_input_editor.read(cx).text(cx);
                    cx.notify();
                }
            },
        );

        // Subscribe to plan title editor events to update plan metadata and sync to frontmatter
        let plan_title_subscription = cx.subscribe_in(
            &plan_title_editor,
            window,
            |panel, _, event, window, cx| {
                if let EditorEvent::BufferEdited { .. } = event {
                    panel.sync_title_to_plan(window, cx);
                    if !panel.suppress_dirty {
                        panel.plan_dirty = true;
                    }
                }
            },
        );

        Self {
            workspace: workspace.weak_handle(),
            focus_handle: cx.focus_handle(),
            state: PlanningState::new(),
            template_registry: Arc::new(TemplateRegistry::new()),
            width: None,
            current_view: PlanningPanelView::PlanList,
            saved_plans: Vec::new(),
            active_plan_id: None,
            plan_input_editor,
            _plan_input_subscription: plan_input_subscription,
            plan_input_description: String::new(),
            template_inference_result: None,
            inferring_templates: false,
            generating_plan: false,
            show_template_selector: false,
            markdown_editor,
            _editor_subscription: editor_subscription,
            plan_title_editor,
            _plan_title_subscription: plan_title_subscription,
            ai_suggestions: Vec::new(),
            next_suggestion_id: 0,
            ai_loading: false,
            _ai_task: None,
            plan_dirty: false,
            suppress_dirty: false,
        }
    }

    /// Sync markdown editor content to plan state
    fn sync_editor_to_plan(&mut self, cx: &mut Context<Self>) {
        let content = self.markdown_editor.read(cx).text(cx);
        if let Some(mut parsed_plan) = parse_markdown_to_plan(&content) {
            // IMPORTANT: Preserve the existing plan ID to avoid creating duplicates
            // parse_markdown_to_plan generates a new ID each time, but we want to
            // keep the original ID so that save_current_plan can find the existing entry
            if let Some(existing_plan) = &self.state.current_plan {
                parsed_plan.id = existing_plan.id;
            }

            // Update the plan state with parsed content
            let event = PlanEvent::PlanCreated {
                plan: parsed_plan,
                timestamp: Utc::now(),
            };
            // Silently update - this is a sync, not a new creation
            let _ = self.state.apply_event(event);
        }
    }

    /// Set markdown editor content from plan
    fn sync_plan_to_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(plan) = &self.state.current_plan {
            let markdown = render_plan_to_markdown(plan);
            // Suppress dirty marking during programmatic update
            self.suppress_dirty = true;
            self.markdown_editor.update(cx, |editor, cx| {
                editor.set_text(markdown, window, cx);
            });
            // Also sync the title editor
            self.plan_title_editor.update(cx, |editor, cx| {
                let current_text = editor.text(cx);
                // Only update if different to avoid cursor jumping
                if let Some(plan) = &self.state.current_plan {
                    if current_text != plan.metadata.title {
                        editor.set_text(plan.metadata.title.clone(), window, cx);
                    }
                }
            });
            self.suppress_dirty = false;
        }
    }

    /// Sync title editor content to plan metadata and update markdown frontmatter
    fn sync_title_to_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_title = self.plan_title_editor.read(cx).text(cx);

        if let Some(plan) = &self.state.current_plan {
            // Only update if title actually changed
            if plan.metadata.title == new_title {
                return;
            }

            // Create updated metadata with new title
            let mut new_metadata = plan.metadata.clone();
            new_metadata.title = new_title;
            new_metadata.updated_at = Utc::now();

            // Apply the metadata update event
            let event = PlanEvent::MetadataUpdated {
                metadata: new_metadata,
                timestamp: Utc::now(),
            };
            if let Err(e) = self.state.apply_event(event) {
                log::error!("Failed to update plan title: {:?}", e);
                return;
            }

            // Sync the updated plan to the markdown editor (to update frontmatter)
            self.sync_plan_to_editor(window, cx);
        }
    }

    /// Create a new plan from a template
    pub fn create_plan_from_template(&mut self, template_name: &str, title: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(template) = self.template_registry.get_template(template_name) {
            let plan = template.instantiate(title.clone(), format!("Created from {} template", template_name));
            let event = PlanEvent::PlanCreated {
                plan,
                timestamp: Utc::now(),
            };
            if let Err(e) = self.state.apply_event(event) {
                log::error!("Failed to create plan: {:?}", e);
            }
            self.show_template_selector = false;
            // Sync plan to markdown editor
            self.sync_plan_to_editor(window, cx);
            cx.notify();
        }
    }

    /// Create an empty plan
    pub fn create_empty_plan(&mut self, title: String, window: &mut Window, cx: &mut Context<Self>) {
        let now = Utc::now();
        let plan = Plan {
            id: PlanId::new(),
            version: PlanVersion::initial(),
            metadata: PlanMetadata {
                title: title.clone(),
                description: String::new(),
                created_at: now,
                updated_at: now,
                template_name: None,
            },
            root: PlanNode::new(NodeType::Goal, "New Plan".to_string()),
        };
        let event = PlanEvent::PlanCreated {
            plan,
            timestamp: now,
        };
        if let Err(e) = self.state.apply_event(event) {
            log::error!("Failed to create plan: {:?}", e);
        }
        self.show_template_selector = false;
        // Sync plan to markdown editor
        self.sync_plan_to_editor(window, cx);
        cx.notify();
    }

    /// Insert text into the markdown editor (used by AI suggestions)
    pub fn insert_markdown(&mut self, text: &str, _window: &mut Window, cx: &mut Context<Self>) {
        self.markdown_editor.update(cx, |editor, cx| {
            // Insert at end of document
            let end = editor.buffer().read(cx).len(cx);
            editor.edit(vec![(end..end, format!("\n{}", text))], cx);
        });
        // Re-sync to plan state
        self.sync_editor_to_plan(cx);
        cx.notify();
    }

    /// Export tasks to markdown and copy to clipboard
    pub fn export_tasks_to_clipboard(&self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(plan) = &self.state.current_plan {
            let tasks = derive_tasks_from_plan(plan);
            let markdown = tasks_to_markdown(&tasks);
            cx.write_to_clipboard(ClipboardItem::new_string(markdown));
        }
    }

    /// Close the current plan and return to empty state
    pub fn close_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.current_plan.is_some() {
            let event = PlanEvent::PlanClosed {
                timestamp: Utc::now(),
            };
            if let Err(e) = self.state.apply_event(event) {
                log::error!("Failed to close plan: {:?}", e);
            }
            // Clear the markdown editor and suggestions
            self.markdown_editor.update(cx, |editor, cx| {
                editor.clear(window, cx);
            });
            self.ai_suggestions.clear();
            cx.notify();
        }
    }

    // --- AI Suggestion Methods ---

    /// Request AI suggestions for the current plan
    pub fn request_ai_suggestions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_loading || self.state.current_plan.is_none() {
            return;
        }

        self.ai_loading = true;
        cx.notify();

        let plan = self.state.current_plan.clone().unwrap();
        let task = cx.spawn_in(window, async move |panel, cx| {
            let result = Self::generate_suggestions_async(&plan, cx).await;

            let _ = panel.update_in(cx, |panel, _window, cx| {
                panel.ai_loading = false;
                match result {
                    Ok(suggestions) => {
                        for suggestion in suggestions {
                            panel.ai_suggestions.push(AiSuggestion {
                                id: panel.next_suggestion_id,
                                description: suggestion.0,
                                target_node: suggestion.1,
                                suggestion_type: suggestion.2,
                                content: suggestion.3,
                            });
                            panel.next_suggestion_id += 1;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to generate AI suggestions: {:?}", e);
                    }
                }
                cx.notify();
            });
        });

        self._ai_task = Some(task);
    }

    /// Generate suggestions using the language model
    async fn generate_suggestions_async(
        plan: &Plan,
        cx: &mut AsyncWindowContext,
    ) -> Result<Vec<(String, Option<NodeId>, AiSuggestionType, String)>> {
        // Get the default language model
        let configured_model = cx.update(|_window, cx| {
            LanguageModelRegistry::read_global(cx).default_model()
        })?;

        let Some(configured_model) = configured_model else {
            return Err(anyhow::anyhow!("No language model configured. Please configure a language model in Settings."));
        };

        // Trigger authentication to load API key if needed
        let auth_task = cx.update(|_window, cx| {
            configured_model.provider.authenticate(cx)
        })?;
        auth_task.await?;

        // Check if the provider is now authenticated
        let is_authenticated = cx.update(|_window, cx| {
            configured_model.provider.is_authenticated(cx)
        })?;

        if !is_authenticated {
            return Err(anyhow::anyhow!(
                "{} provider is not authenticated. Please configure your API key in Settings.",
                configured_model.provider.name().0
            ));
        }

        let model = configured_model.model;

        // Build the prompt
        let plan_text = Self::plan_to_prompt_text(plan);
        let prompt = format!(
            r#"You are a planning assistant. Analyze this plan and suggest improvements.

Current Plan:
{}

Provide 2-3 specific, actionable suggestions to improve this plan. For each suggestion:
1. Describe what to improve (one sentence)
2. Provide the specific content to add or change

Format each suggestion on a single line as:
SUGGESTION: [description] | [content to add/change]

Focus on:
- Missing steps or details
- Unclear or vague items
- Potential risks or constraints not addressed
- Dependencies or ordering issues"#,
            plan_text
        );

        let request = LanguageModelRequest {
            thread_id: None,
            prompt_id: None,
            intent: None,
            messages: vec![LanguageModelRequestMessage {
                role: Role::User,
                content: vec![language_model::MessageContent::Text(prompt)],
                cache: false,
                reasoning_details: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            stop: Vec::new(),
            temperature: None, // Let model use default (some models like o1/o3 don't support temperature)
            thinking_allowed: false,
            thinking_effort: None,
        };

        // Stream the completion - use deref to get AsyncApp reference
        let async_app: &gpui::AsyncApp = &*cx;
        let stream_result = model.stream_completion_text(request, async_app).await?;
        let mut stream = stream_result.stream;
        let mut response = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => response.push_str(&text),
                Err(e) => {
                    log::warn!("Error in AI stream: {:?}", e);
                    break;
                }
            }
        }

        // Parse suggestions from response
        let mut suggestions = Vec::new();
        for line in response.lines() {
            if line.starts_with("SUGGESTION:") {
                let content = line.trim_start_matches("SUGGESTION:").trim();
                if let Some((desc, suggestion_content)) = content.split_once('|') {
                    suggestions.push((
                        desc.trim().to_string(),
                        None, // Target node determined later
                        AiSuggestionType::Critique,
                        suggestion_content.trim().to_string(),
                    ));
                }
            }
        }

        Ok(suggestions)
    }

    /// Convert plan to text for the prompt
    fn plan_to_prompt_text(plan: &Plan) -> String {
        let mut text = format!("# {}\n\n", plan.metadata.title);
        Self::node_to_prompt_text(&plan.root, 0, &mut text);
        text
    }

    /// Convert a node to text recursively
    fn node_to_prompt_text(node: &PlanNode, depth: usize, text: &mut String) {
        let indent = "  ".repeat(depth);
        let type_label = match node.node_type {
            NodeType::Goal => "[GOAL]",
            NodeType::Phase => "[PHASE]",
            NodeType::Task => "[TASK]",
            NodeType::Constraint => "[CONSTRAINT]",
            NodeType::Assumption => "[ASSUMPTION]",
            NodeType::Decision => "[DECISION]",
            NodeType::Note => "[NOTE]",
        };
        text.push_str(&format!("{}{} {}\n", indent, type_label, node.content));
        for child in &node.children {
            Self::node_to_prompt_text(child, depth + 1, text);
        }
    }

    /// Accept an AI suggestion - inserts as markdown
    pub fn accept_suggestion(&mut self, suggestion_id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(idx) = self.ai_suggestions.iter().position(|s| s.id == suggestion_id) {
            let suggestion = self.ai_suggestions.remove(idx);

            // Format the suggestion as markdown and insert it
            let markdown_text = match suggestion.suggestion_type {
                AiSuggestionType::AddChild | AiSuggestionType::AddSibling => {
                    format!("- {}", suggestion.content)
                }
                AiSuggestionType::UpdateContent | AiSuggestionType::Critique => {
                    format!("- Note: {}", suggestion.content)
                }
            };

            self.insert_markdown(&markdown_text, window, cx);
            cx.notify();
        }
    }

    /// Dismiss an AI suggestion
    pub fn dismiss_suggestion(&mut self, suggestion_id: usize, cx: &mut Context<Self>) {
        self.ai_suggestions.retain(|s| s.id != suggestion_id);
        cx.notify();
    }

    /// Clear all AI suggestions
    #[allow(dead_code)]
    pub fn clear_suggestions(&mut self, cx: &mut Context<Self>) {
        self.ai_suggestions.clear();
        cx.notify();
    }

    // --- AI Plan Generation Methods (Phase 3 & 4) ---

    /// Start the plan generation workflow
    /// This first infers templates from user input, then generates a full plan
    fn start_plan_generation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Get the user input from the editor
        let description = self.plan_input_editor.read(cx).text(cx);
        if description.trim().is_empty() {
            return;
        }

        self.plan_input_description = description.clone();
        self.inferring_templates = true;
        self.template_inference_result = None;
        cx.notify();

        // Get available template names
        let template_names: Vec<String> = self.template_registry
            .list_templates()
            .iter()
            .map(|t| t.name.clone())
            .collect();

        let task = cx.spawn_in(window, async move |panel, cx| {
            // Phase 3: Infer templates from user input
            let inference_result = Self::infer_templates_async(&description, &template_names, cx).await;

            // Handle inference result - use default on failure
            let inference = match inference_result {
                Ok(result) => result,
                Err(e) => {
                    log::error!("Failed to infer templates: {:?}", e);
                    TemplateInferenceResult {
                        templates: vec!["Feature Development".to_string()],
                        explanation: format!("Using default template (inference failed: {})", e),
                    }
                }
            };

            // Phase 4: Generate the full plan
            let plan_result = Self::generate_plan_async(&description, &inference, cx).await;

            // Final update_in call with all state changes
            let _ = panel.update_in(cx, |panel, window, cx| {
                panel.inferring_templates = false;
                panel.generating_plan = false;
                panel.template_inference_result = Some(inference);

                match plan_result {
                    Ok(markdown_content) => {
                        // Parse the generated markdown into a Plan
                        if let Some(plan) = parse_markdown_to_plan(&markdown_content) {
                            let plan_id = plan.id;
                            let event = PlanEvent::PlanCreated {
                                plan: plan.clone(),
                                timestamp: Utc::now(),
                            };

                            // Store title for the title editor
                            let plan_title = plan.metadata.title.clone();

                            if let Err(e) = panel.state.apply_event(event) {
                                log::error!("Failed to create plan: {:?}", e);
                                return;
                            }

                            // Set the markdown in the editor
                            // Suppress dirty marking during programmatic update
                            panel.suppress_dirty = true;
                            panel.markdown_editor.update(cx, |editor, cx| {
                                editor.set_text(markdown_content.clone(), window, cx);
                            });
                            // Also set the title editor
                            panel.plan_title_editor.update(cx, |editor, cx| {
                                editor.set_text(plan_title, window, cx);
                            });
                            panel.suppress_dirty = false;

                            // Save the new plan
                            let now = Utc::now();
                            panel.saved_plans.push(SavedPlanSummary {
                                id: plan_id,
                                name: plan.metadata.title.clone(),
                                description: plan.metadata.description.clone(),
                                created_at: now,
                                updated_at: now,
                                content: markdown_content,
                            });

                            panel.active_plan_id = Some(plan_id);
                            panel.current_view = PlanningPanelView::PlanEditor;
                            panel.plan_dirty = false; // Plan just created and saved
                            panel.serialize(cx);
                        } else {
                            log::error!("Failed to parse generated markdown into plan");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to generate plan: {:?}", e);
                    }
                }
                cx.notify();
            });
        });

        self._ai_task = Some(task);
    }

    /// Infer which templates to use based on user's description
    async fn infer_templates_async(
        description: &str,
        available_templates: &[String],
        cx: &mut AsyncWindowContext,
    ) -> Result<TemplateInferenceResult> {
        let configured_model = cx.update(|_window, cx| {
            LanguageModelRegistry::read_global(cx).default_model()
        })?;

        let Some(configured_model) = configured_model else {
            return Err(anyhow::anyhow!("No language model configured. Please configure a language model in Settings."));
        };

        // Trigger authentication to load API key if needed
        let auth_task = cx.update(|_window, cx| {
            configured_model.provider.authenticate(cx)
        })?;
        auth_task.await?;

        // Check if the provider is now authenticated
        let is_authenticated = cx.update(|_window, cx| {
            configured_model.provider.is_authenticated(cx)
        })?;

        if !is_authenticated {
            return Err(anyhow::anyhow!(
                "{} provider is not authenticated. Please configure your API key in Settings.",
                configured_model.provider.name().0
            ));
        }

        let model = configured_model.model;

        let templates_list = available_templates.join(", ");
        let prompt = format!(
            r#"You are a planning assistant. Based on the user's description, select the most appropriate templates from the available options.

Available templates: {}

User's request:
{}

Analyze the request and select 1-3 templates that best match the user's intent. Respond in this exact format:
TEMPLATES: [comma-separated template names]
EXPLANATION: [brief explanation of why these templates were selected]

Only select templates that are in the available list above."#,
            templates_list,
            description
        );

        let request = LanguageModelRequest {
            thread_id: None,
            prompt_id: None,
            intent: None,
            messages: vec![LanguageModelRequestMessage {
                role: Role::User,
                content: vec![language_model::MessageContent::Text(prompt)],
                cache: false,
                reasoning_details: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            stop: Vec::new(),
            temperature: None, // Let model use default (some models like o1/o3 don't support temperature)
            thinking_allowed: false,
            thinking_effort: None,
        };

        let async_app: &gpui::AsyncApp = &*cx;
        let stream_result = model.stream_completion_text(request, async_app).await?;
        let mut stream = stream_result.stream;
        let mut response = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => response.push_str(&text),
                Err(e) => {
                    log::warn!("Error in AI stream: {:?}", e);
                    break;
                }
            }
        }

        // Parse the response
        let mut templates = Vec::new();
        let mut explanation = String::new();

        for line in response.lines() {
            if line.starts_with("TEMPLATES:") {
                let content = line.trim_start_matches("TEMPLATES:").trim();
                templates = content
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && available_templates.contains(s))
                    .collect();
            } else if line.starts_with("EXPLANATION:") {
                explanation = line.trim_start_matches("EXPLANATION:").trim().to_string();
            }
        }

        // Fallback if no valid templates were found
        if templates.is_empty() {
            templates.push("Feature Development".to_string());
            explanation = "Using default template".to_string();
        }

        Ok(TemplateInferenceResult {
            templates,
            explanation,
        })
    }

    /// Generate a full plan markdown from user description and selected templates
    async fn generate_plan_async(
        description: &str,
        inference: &TemplateInferenceResult,
        cx: &mut AsyncWindowContext,
    ) -> Result<String> {
        let configured_model = cx.update(|_window, cx| {
            LanguageModelRegistry::read_global(cx).default_model()
        })?;

        let Some(configured_model) = configured_model else {
            return Err(anyhow::anyhow!("No language model configured. Please configure a language model in Settings."));
        };

        // Trigger authentication to load API key if needed
        let auth_task = cx.update(|_window, cx| {
            configured_model.provider.authenticate(cx)
        })?;
        auth_task.await?;

        // Check if the provider is now authenticated
        let is_authenticated = cx.update(|_window, cx| {
            configured_model.provider.is_authenticated(cx)
        })?;

        if !is_authenticated {
            return Err(anyhow::anyhow!(
                "{} provider is not authenticated. Please configure your API key in Settings.",
                configured_model.provider.name().0
            ));
        }

        let model = configured_model.model;

        let templates_str = inference.templates.join(", ");
        let prompt = format!(
            r#"You are a planning assistant. Create a detailed, actionable plan based on the user's request.

User's request:
{}

Selected templates: {}

Generate a plan in markdown format with YAML frontmatter. The plan should:
1. Have a clear, descriptive title
2. Include a brief description
3. Be organized into phases with tasks
4. Be specific and actionable

Use this exact format:
```yaml
---
title: [Plan Title]
description: [Brief description]
---
```

# [Plan Title]

## Phase 1: [Phase Name]
- [ ] Task 1
- [ ] Task 2

## Phase 2: [Phase Name]
- [ ] Task 1
- [ ] Task 2

Include relevant details, acceptance criteria, and notes where appropriate.
Generate a comprehensive plan that the user can immediately start working from."#,
            description,
            templates_str
        );

        let request = LanguageModelRequest {
            thread_id: None,
            prompt_id: None,
            intent: None,
            messages: vec![LanguageModelRequestMessage {
                role: Role::User,
                content: vec![language_model::MessageContent::Text(prompt)],
                cache: false,
                reasoning_details: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            stop: Vec::new(),
            temperature: None, // Let model use default (some models like o1/o3 don't support temperature)
            thinking_allowed: false,
            thinking_effort: None,
        };

        let async_app: &gpui::AsyncApp = &*cx;
        let stream_result = model.stream_completion_text(request, async_app).await?;
        let mut stream = stream_result.stream;
        let mut response = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => response.push_str(&text),
                Err(e) => {
                    log::warn!("Error in AI stream: {:?}", e);
                    break;
                }
            }
        }

        // Clean up the response - remove markdown code blocks if present
        let markdown = response
            .trim()
            .trim_start_matches("```yaml")
            .trim_start_matches("```markdown")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string();

        Ok(markdown)
    }

    fn serialize(&mut self, cx: &mut Context<Self>) {
        let Some(serialization_key) = self
            .workspace
            .read_with(cx, |workspace, _| {
                PlanningPanel::serialization_key(workspace)
            })
            .ok()
            .flatten()
        else {
            return;
        };

        let width = self.width.map(|w| f32::from(w));
        let saved_plans = self.saved_plans.clone();
        let active_plan_id = self.active_plan_id;

        cx.background_spawn(async move {
            KEY_VALUE_STORE
                .write_kvp(
                    serialization_key,
                    serde_json::to_string(&SerializedPlanningPanel {
                        width,
                        saved_plans,
                        active_plan_id,
                    }).unwrap_or_default(),
                )
                .await
                .ok();
        })
        .detach();
    }

    /// Save the current plan to the saved plans list
    fn save_current_plan(&mut self, cx: &mut Context<Self>) {
        if let Some(plan) = &self.state.current_plan {
            let content = self.markdown_editor.read(cx).text(cx);
            let now = Utc::now();

            // Check if we're updating an existing plan or creating a new one
            if let Some(existing_idx) = self.saved_plans.iter().position(|p| p.id == plan.id) {
                // Update existing plan
                self.saved_plans[existing_idx] = SavedPlanSummary {
                    id: plan.id,
                    name: plan.metadata.title.clone(),
                    description: plan.metadata.description.clone(),
                    created_at: self.saved_plans[existing_idx].created_at,
                    updated_at: now,
                    content,
                };
            } else {
                // Add new plan
                self.saved_plans.push(SavedPlanSummary {
                    id: plan.id,
                    name: plan.metadata.title.clone(),
                    description: plan.metadata.description.clone(),
                    created_at: now,
                    updated_at: now,
                    content,
                });
            }

            self.active_plan_id = Some(plan.id);
            self.plan_dirty = false;
            self.serialize(cx);
        }
    }

    /// Load a saved plan by ID
    fn load_saved_plan(&mut self, plan_id: PlanId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(saved_plan) = self.saved_plans.iter().find(|p| p.id == plan_id).cloned() {
            if let Some(mut plan) = parse_markdown_to_plan(&saved_plan.content) {
                // IMPORTANT: Preserve the original plan ID from saved_plans
                // parse_markdown_to_plan generates a new ID, but we need to keep
                // the original so that save_current_plan can find the existing entry
                plan.id = plan_id;

                // Store title for the title editor
                let plan_title = plan.metadata.title.clone();

                let event = PlanEvent::PlanCreated {
                    plan,
                    timestamp: Utc::now(),
                };
                if let Err(e) = self.state.apply_event(event) {
                    log::error!("Failed to load plan: {:?}", e);
                    return;
                }
                // Suppress dirty marking during programmatic update
                self.suppress_dirty = true;
                self.markdown_editor.update(cx, |editor, cx| {
                    editor.set_text(saved_plan.content.clone(), window, cx);
                });
                // Also set the title editor
                self.plan_title_editor.update(cx, |editor, cx| {
                    editor.set_text(plan_title, window, cx);
                });
                self.suppress_dirty = false;
                self.active_plan_id = Some(plan_id);
                self.current_view = PlanningPanelView::PlanEditor;
                self.plan_dirty = false; // Plan just loaded, no unsaved changes
                self.serialize(cx);
                cx.notify();
            }
        }
    }

    /// Delete a saved plan by ID
    fn delete_saved_plan(&mut self, plan_id: PlanId, cx: &mut Context<Self>) {
        self.saved_plans.retain(|p| p.id != plan_id);
        if self.active_plan_id == Some(plan_id) {
            self.active_plan_id = None;
        }
        self.serialize(cx);
        cx.notify();
    }

    /// Navigate to the new plan dialog
    fn show_new_plan_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.current_view = PlanningPanelView::NewPlanDialog;
        self.plan_input_description.clear();
        self.plan_input_editor.update(cx, |editor, cx| {
            editor.clear(window, cx);
        });
        self.template_inference_result = None;
        self.inferring_templates = false;
        self.generating_plan = false;
        cx.notify();
    }

    /// Navigate back to the plan list
    fn show_plan_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Save current plan if editing and dirty (has unsaved changes)
        if self.current_view == PlanningPanelView::PlanEditor && self.state.current_plan.is_some() && self.plan_dirty {
            self.save_current_plan(cx);
        }

        // Clear the current plan state
        if self.state.current_plan.is_some() {
            let event = PlanEvent::PlanClosed {
                timestamp: Utc::now(),
            };
            let _ = self.state.apply_event(event);
        }

        self.markdown_editor.update(cx, |editor, cx| {
            editor.clear(window, cx);
        });
        self.ai_suggestions.clear();
        self.active_plan_id = None;
        self.current_view = PlanningPanelView::PlanList;
        self.serialize(cx);
        cx.notify();
    }
}

impl EventEmitter<PanelEvent> for PlanningPanel {}
impl EventEmitter<DismissEvent> for PlanningPanel {}

impl Focusable for PlanningPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PlanningPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_view = self.current_view.clone();
        let has_suggestions = !self.ai_suggestions.is_empty();

        v_flex()
            .id("planning-panel")
            .key_context("PlanningPanel")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().colors().panel_background)
            // Header
            .child(self.render_header(cx))
            // Content based on current view
            .child(
                v_flex()
                    .id("planning-panel-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .map(|el| match current_view {
                        PlanningPanelView::PlanList => {
                            el.child(self.render_plan_list(cx))
                        }
                        PlanningPanelView::NewPlanDialog => {
                            el.child(self.render_new_plan_dialog(cx))
                        }
                        PlanningPanelView::PlanEditor => {
                            el.child(
                                v_flex()
                                    .size_full()
                                    .child(self.render_plan_title_editor(cx))
                                    .child(self.markdown_editor.clone())
                            )
                        }
                    })
            )
            // AI Suggestions section at the bottom (only in editor view)
            .when(has_suggestions && current_view == PlanningPanelView::PlanEditor, |el| {
                el.child(self.render_ai_suggestions(cx))
            })
    }
}

impl PlanningPanel {
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_view = self.current_view.clone();
        let ai_loading = self.ai_loading || self.inferring_templates || self.generating_plan;

        let header_title = match &current_view {
            PlanningPanelView::PlanList => "Plans",
            PlanningPanelView::NewPlanDialog => "New Plan",
            PlanningPanelView::PlanEditor => "Edit Plan",
        };

        let loading_text = if self.inferring_templates {
            Some("Analyzing...")
        } else if self.generating_plan {
            Some("Generating plan...")
        } else if self.ai_loading {
            Some("AI thinking...")
        } else {
            None
        };

        h_flex()
            .w_full()
            .p_2()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    // Back button for non-list views
                    .when(current_view != PlanningPanelView::PlanList, |el| {
                        el.child(
                            IconButton::new("back-to-list", IconName::ArrowLeft)
                                .icon_size(IconSize::Small)
                                .tooltip(Tooltip::text("Back to Plans"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_plan_list(window, cx);
                                }))
                        )
                    })
                    .child(Label::new(header_title).size(LabelSize::Large))
                    .when_some(loading_text, |el, text| {
                        el.child(Label::new(text).size(LabelSize::Small).color(Color::Muted))
                    })
            )
            .child(
                h_flex()
                    .gap_1()
                    // Plan list view buttons
                    .when(current_view == PlanningPanelView::PlanList, |el| {
                        el.child(
                            IconButton::new("new-plan", IconName::Plus)
                                .icon_size(IconSize::Small)
                                .tooltip(Tooltip::text("New Plan"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_new_plan_dialog(window, cx);
                                }))
                        )
                    })
                    // Editor view buttons
                    .when(current_view == PlanningPanelView::PlanEditor, |el| {
                        el.child(
                            IconButton::new("ai-suggest", IconName::Sparkle)
                                .icon_size(IconSize::Small)
                                .tooltip(Tooltip::text("Get AI Suggestions"))
                                .disabled(ai_loading)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_ai_suggestions(window, cx);
                                }))
                        )
                        .child(
                            IconButton::new("export-tasks", IconName::Copy)
                                .icon_size(IconSize::Small)
                                .tooltip(Tooltip::text("Export Tasks to Clipboard"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.export_tasks_to_clipboard(window, cx);
                                }))
                        )
                        .child(
                            IconButton::new("save-plan", IconName::Check)
                                .icon_size(IconSize::Small)
                                .tooltip(Tooltip::text("Save Plan"))
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.save_current_plan(cx);
                                }))
                        )
                    })
            )
    }

    /// Render the plan title editor above the markdown editor
    fn render_plan_title_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .px_3()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().colors().border_variant)
            .bg(cx.theme().colors().surface_background)
            .child(
                Icon::new(IconName::FileDoc)
                    .size(IconSize::Small)
                    .color(Color::Muted)
            )
            .child(
                div()
                    .flex_1()
                    .child(self.plan_title_editor.clone())
            )
    }

    /// Render the plan list view (Phase 1)
    fn render_plan_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_plans = !self.saved_plans.is_empty();

        v_flex()
            .p_4()
            .gap_3()
            .size_full()
            .when(!has_plans, |el| {
                // Empty state
                el.child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_4()
                        .child(
                            Icon::new(IconName::ListTree)
                                .size(IconSize::XLarge)
                                .color(Color::Muted)
                        )
                        .child(
                            Label::new("No plans yet")
                                .size(LabelSize::Large)
                                .color(Color::Muted)
                        )
                        .child(
                            Label::new("Create a new plan to get started")
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                        )
                        .child(
                            Button::new("create-first-plan", "+ New Plan")
                                .style(ButtonStyle::Filled)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_new_plan_dialog(window, cx);
                                }))
                        )
                )
            })
            .when(has_plans, |el| {
                // Plan list
                el.child(
                    v_flex()
                        .gap_2()
                        .children(self.saved_plans.iter().map(|plan| {
                            let plan_id = plan.id;
                            let plan_name = plan.name.clone();
                            let plan_description = plan.description.clone();
                            let updated_at = plan.updated_at.format("%Y-%m-%d %H:%M").to_string();

                            ListItem::new(SharedString::from(format!("plan-{:?}", plan_id)))
                                .start_slot(Icon::new(IconName::FileDoc).size(IconSize::Small))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .overflow_hidden()
                                        .child(
                                            Label::new(if plan_name.is_empty() { "Untitled Plan".to_string() } else { plan_name })
                                                .size(LabelSize::Default)
                                        )
                                        .when(!plan_description.is_empty(), |el| {
                                            el.child(
                                                Label::new(plan_description)
                                                    .size(LabelSize::Small)
                                                    .color(Color::Muted)
                                            )
                                        })
                                        .child(
                                            Label::new(format!("Updated: {}", updated_at))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted)
                                        )
                                )
                                .end_slot(
                                    IconButton::new(SharedString::from(format!("delete-plan-{:?}", plan_id)), IconName::Trash)
                                        .icon_size(IconSize::Small)
                                        .tooltip(Tooltip::text("Delete Plan"))
                                        .on_click(cx.listener(move |this, _, _window, cx| {
                                            this.delete_saved_plan(plan_id, cx);
                                        }))
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.load_saved_plan(plan_id, window, cx);
                                }))
                        }))
                )
                .child(
                    h_flex()
                        .pt_3()
                        .border_t_1()
                        .border_color(cx.theme().colors().border)
                        .child(
                            Button::new("new-plan-btn", "+ New Plan")
                                .style(ButtonStyle::Filled)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_new_plan_dialog(window, cx);
                                }))
                        )
                )
            })
    }

    /// Render the new plan dialog (Phase 2)
    fn render_new_plan_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let _has_inference_result = self.template_inference_result.is_some();
        let is_loading = self.inferring_templates || self.generating_plan;
        let input_is_empty = self.plan_input_description.trim().is_empty();

        v_flex()
            .p_4()
            .gap_4()
            .size_full()
            // Instructions
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Label::new("What do you want to plan?")
                            .size(LabelSize::Large)
                    )
                    .child(
                        Label::new("Describe your goal in plain language. The AI will help structure it into an actionable plan.")
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                    )
            )
            // Input editor
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .id("plan-input-container")
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .bg(cx.theme().colors().editor_background)
                            .p_2()
                            .min_h(px(200.0))
                            .max_h(px(400.0))
                            .overflow_y_scroll()
                            .child(self.plan_input_editor.clone())
                    )
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-plan", "Cancel")
                                    .style(ButtonStyle::Subtle)
                                    .disabled(is_loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.show_plan_list(window, cx);
                                    }))
                            )
                            .child(
                                Button::new("generate-plan", "Generate Plan")
                                    .icon(IconName::Sparkle)
                                    .icon_position(IconPosition::Start)
                                    .style(ButtonStyle::Filled)
                                    .disabled(input_is_empty || is_loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_plan_generation(window, cx);
                                    }))
                            )
                    )
            )
            // Template inference result (Phase 3)
            .when_some(self.template_inference_result.clone(), |el, result| {
                el.child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_md()
                        .bg(cx.theme().colors().surface_background)
                        .border_1()
                        .border_color(cx.theme().colors().border_variant)
                        .child(
                            h_flex()
                                .gap_2()
                                .child(Icon::new(IconName::Sparkle).size(IconSize::Small).color(Color::Accent))
                                .child(Label::new("Selected Templates").size(LabelSize::Small))
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .flex_wrap()
                                .children(result.templates.iter().map(|t| {
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(cx.theme().colors().element_background)
                                        .child(Label::new(t.clone()).size(LabelSize::Small))
                                }))
                        )
                        .child(
                            Label::new(result.explanation)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                        )
                )
            })
            // Loading indicator
            .when(is_loading, |el| {
                el.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .justify_center()
                        .p_4()
                        .child(
                            Icon::new(IconName::ArrowCircle)
                                .size(IconSize::Small)
                                .color(Color::Accent)
                        )
                        .child(
                            Label::new(if self.inferring_templates {
                                "Analyzing your request..."
                            } else {
                                "Generating your plan..."
                            })
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                        )
                )
            })
    }

    #[allow(dead_code)]
    fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p_4()
            .gap_4()
            .child(
                Icon::new(IconName::ListTree)
                    .size(IconSize::XLarge)
                    .color(Color::Muted)
            )
            .child(
                Label::new("No plan yet")
                    .size(LabelSize::Large)
                    .color(Color::Muted)
            )
            .child(
                Label::new("Create a new plan to get started")
                    .size(LabelSize::Small)
                    .color(Color::Muted)
            )
            .child(
                Button::new("create-plan", "New Plan")
                    .icon(IconName::Plus)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_new_plan_dialog(window, cx);
                    }))
            )
    }

    #[allow(dead_code)]
    fn render_template_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let templates = self.template_registry.list_templates();

        v_flex()
            .p_4()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .child(Label::new("Select a Template").size(LabelSize::Large))
                    .child(
                        IconButton::new("close-selector", IconName::Close)
                            .icon_size(IconSize::Small)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.show_template_selector = false;
                                cx.notify();
                            }))
                    )
            )
            .child(
                v_flex()
                    .gap_1()
                    .children(templates.into_iter().map(|template| {
                        let name = template.name.clone();
                        let description = template.description.clone();
                        let template_name = template.name.clone();

                        ListItem::new(SharedString::from(format!("template-{}", name)))
                            .start_slot(Icon::new(IconName::FileDoc).size(IconSize::Small))
                            .child(
                                v_flex()
                                    .child(Label::new(name))
                                    .child(Label::new(description).size(LabelSize::Small).color(Color::Muted))
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.create_plan_from_template(&template_name, "New Plan".to_string(), window, cx);
                            }))
                    }))
            )
            .child(
                h_flex()
                    .pt_2()
                    .border_t_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        Button::new("empty-plan", "Empty Plan")
                            .icon(IconName::File)
                            .icon_position(IconPosition::Start)
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_empty_plan("New Plan".to_string(), window, cx);
                            }))
                    )
            )
    }

    /// Render the AI suggestions section
    fn render_ai_suggestions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .p_2()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        Label::new("AI Suggestions")
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                    )
                    .child(
                        Label::new(format!("{} pending", self.ai_suggestions.len()))
                            .size(LabelSize::XSmall)
                            .color(Color::Muted)
                    )
            )
            .children(
                self.ai_suggestions.iter().map(|suggestion| {
                    let id = suggestion.id;
                    let description = suggestion.description.clone();
                    let content = suggestion.content.clone();

                    v_flex()
                        .p_2()
                        .gap_1()
                        .rounded_md()
                        .bg(cx.theme().colors().surface_background)
                        .border_1()
                        .border_color(cx.theme().colors().border_variant)
                        .child(
                            Label::new(description)
                                .size(LabelSize::Small)
                                .color(Color::Default)
                        )
                        .child(
                            Label::new(content)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .pt_1()
                                .child(
                                    Button::new(SharedString::from(format!("accept-{}", id)), "Accept")
                                        .style(ButtonStyle::Filled)
                                        .size(ButtonSize::Compact)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.accept_suggestion(id, window, cx);
                                        }))
                                )
                                .child(
                                    Button::new(SharedString::from(format!("dismiss-{}", id)), "Dismiss")
                                        .style(ButtonStyle::Subtle)
                                        .size(ButtonSize::Compact)
                                        .on_click(cx.listener(move |this, _, _window, cx| {
                                            this.dismiss_suggestion(id, cx);
                                        }))
                                )
                        )
                })
            )
    }
}

impl Panel for PlanningPanel {
    fn persistent_name() -> &'static str {
        "PlanningPanel"
    }

    fn panel_key() -> &'static str {
        PLANNING_PANEL_KEY
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Right
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Right | DockPosition::Left)
    }

    fn set_position(&mut self, _position: DockPosition, _window: &mut Window, cx: &mut Context<Self>) {
        self.serialize(cx);
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or_else(|| px(400.0))
    }

    fn set_size(&mut self, size: Option<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.width = size;
        cx.notify();
        cx.defer_in(window, |this, _, cx| {
            this.serialize(cx);
        });
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<ui::IconName> {
        Some(ui::IconName::ListTree)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Planning Panel")
    }

    fn toggle_action(&self) -> Box<dyn gpui::Action> {
        Box::new(crate::actions::ToggleFocus)
    }

    fn activation_priority(&self) -> u32 {
        6 // After outline panel (5)
    }
}

