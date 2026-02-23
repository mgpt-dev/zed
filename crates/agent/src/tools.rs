mod context_server_registry;
mod copy_path_tool;
mod create_directory_tool;
mod delete_path_tool;
mod diagnostics_tool;
mod edit_file_tool;
mod fetch_tool;
mod find_path_tool;
mod grep_tool;
mod list_directory_tool;
mod move_path_tool;
mod now_tool;
mod open_tool;
mod read_file_tool;
mod restore_file_from_disk_tool;
mod save_file_tool;
mod spawn_agent_tool;
mod streaming_edit_file_tool;
mod terminal_tool;
mod tool_permissions;
mod web_search_tool;

use crate::AgentTool;
use language_model::{LanguageModelRequestTool, LanguageModelToolSchemaFormat};

pub use context_server_registry::*;
pub use copy_path_tool::*;
pub use create_directory_tool::*;
pub use delete_path_tool::*;
pub use diagnostics_tool::*;
pub use edit_file_tool::*;
pub use fetch_tool::*;
pub use find_path_tool::*;
pub use grep_tool::*;
pub use list_directory_tool::*;
pub use move_path_tool::*;
pub use now_tool::*;
pub use open_tool::*;
pub use read_file_tool::*;
pub use restore_file_from_disk_tool::*;
pub use save_file_tool::*;
pub use spawn_agent_tool::*;
pub use streaming_edit_file_tool::*;
pub use terminal_tool::*;
pub use tool_permissions::*;
pub use web_search_tool::*;

macro_rules! tools {
    ($($tool:ty),* $(,)?) => {
        /// Every built-in tool name, determined at compile time.
        pub const ALL_TOOL_NAMES: &[&str] = &[
            $(<$tool>::NAME,)*
        ];

        const _: () = {
            const fn str_eq(a: &str, b: &str) -> bool {
                let a = a.as_bytes();
                let b = b.as_bytes();
                if a.len() != b.len() {
                    return false;
                }
                let mut i = 0;
                while i < a.len() {
                    if a[i] != b[i] {
                        return false;
                    }
                    i += 1;
                }
                true
            }

            const NAMES: &[&str] = ALL_TOOL_NAMES;
            let mut i = 0;
            while i < NAMES.len() {
                let mut j = i + 1;
                while j < NAMES.len() {
                    if str_eq(NAMES[i], NAMES[j]) {
                        panic!("Duplicate tool name in tools! macro");
                    }
                    j += 1;
                }
                i += 1;
            }
        };

        /// Returns whether the tool with the given name supports the given provider.
        pub fn tool_supports_provider(name: &str, provider: &language_model::LanguageModelProviderId) -> bool {
            $(
                if name == <$tool>::NAME {
                    return <$tool>::supports_provider(provider);
                }
            )*
            false
        }

        /// A list of all built-in tools
        pub fn built_in_tools() -> impl Iterator<Item = LanguageModelRequestTool> {
            fn language_model_tool<T: AgentTool>() -> LanguageModelRequestTool {
                LanguageModelRequestTool {
                    name: T::NAME.to_string(),
                    description: T::description().to_string(),
                    input_schema: T::input_schema(LanguageModelToolSchemaFormat::JsonSchema).to_value(),
                }
            }
            [
                $(
                    language_model_tool::<$tool>(),
                )*
            ]
            .into_iter()
        }
    };
}

tools! {
    CopyPathTool,
    CreateDirectoryTool,
    DeletePathTool,
    DiagnosticsTool,
    EditFileTool,
    FetchTool,
    FindPathTool,
    GrepTool,
    ListDirectoryTool,
    MovePathTool,
    NowTool,
    OpenTool,
    ReadFileTool,
    RestoreFileFromDiskTool,
    SaveFileTool,
    SpawnAgentTool,
    TerminalTool,
    WebSearchTool,
}

use crate::{AnyAgentTool, ToolCallEventStream};
use gpui::Entity;
use project::Project;
use std::collections::HashMap;
use std::sync::Arc;

/// Names of tools available for planning context gathering (read-only exploration tools)
pub const PLANNING_TOOL_NAMES: &[&str] = &[
    GrepTool::NAME,
    FindPathTool::NAME,
    ListDirectoryTool::NAME,
    PlanningReadFileTool::NAME,
];

/// Returns the LanguageModelRequestTool definitions for planning tools.
/// These are the read-only exploration tools used by the planning panel.
pub fn planning_tool_definitions() -> Vec<LanguageModelRequestTool> {
    fn tool_def<T: AgentTool>() -> LanguageModelRequestTool {
        LanguageModelRequestTool {
            name: T::NAME.to_string(),
            description: T::description().to_string(),
            input_schema: T::input_schema(LanguageModelToolSchemaFormat::JsonSchema).to_value(),
        }
    }

    vec![
        tool_def::<GrepTool>(),
        tool_def::<FindPathTool>(),
        tool_def::<ListDirectoryTool>(),
        tool_def::<PlanningReadFileTool>(),
    ]
}

/// A collection of planning tools with their instances for execution.
pub struct PlanningTools {
    tools: HashMap<&'static str, Arc<dyn AnyAgentTool>>,
}

impl PlanningTools {
    /// Creates a new set of planning tools for the given project.
    pub fn new(project: Entity<Project>) -> Self {
        let mut tools: HashMap<&'static str, Arc<dyn AnyAgentTool>> = HashMap::default();
        tools.insert(GrepTool::NAME, GrepTool::new(project.clone()).erase());
        tools.insert(FindPathTool::NAME, FindPathTool::new(project.clone()).erase());
        tools.insert(ListDirectoryTool::NAME, ListDirectoryTool::new(project.clone()).erase());
        tools.insert(
            PlanningReadFileTool::NAME,
            PlanningReadFileTool::new(project).erase(),
        );
        Self { tools }
    }

    /// Returns the tool definitions for use in LanguageModelRequest.
    pub fn definitions(&self) -> Vec<LanguageModelRequestTool> {
        planning_tool_definitions()
    }

    /// Gets a tool by name for execution.
    pub fn get(&self, name: &str) -> Option<Arc<dyn AnyAgentTool>> {
        self.tools.get(name).cloned()
    }

    /// Runs a tool and returns the result.
    /// This is a convenience method for running planning tools without UI event emission.
    pub fn run_tool(
        &self,
        tool_use: &language_model::LanguageModelToolUse,
        fs: Option<Arc<dyn fs::Fs>>,
        cx: &mut gpui::App,
    ) -> Option<gpui::Task<language_model::LanguageModelToolResult>> {
        let tool = self.get(&tool_use.name)?;
        let event_stream = ToolCallEventStream::no_op(tool_use.id.clone(), fs);
        let tool_result = tool.run(tool_use.input.clone(), event_stream, cx);
        let tool_use_id = tool_use.id.clone();
        let tool_name = tool_use.name.clone();

        Some(cx.foreground_executor().spawn(async move {
            let (is_error, content) = match tool_result.await {
                Ok(output) => (false, output.llm_output),
                Err(output) => (true, output.llm_output),
            };

            language_model::LanguageModelToolResult {
                tool_use_id,
                tool_name,
                is_error,
                content,
                output: None,
            }
        }))
    }
}

// PlanningReadFileTool: A simplified read_file tool for planning that doesn't require Thread or ActionLog
use crate::outline;
use agent_client_protocol as acp;
use anyhow::anyhow;
use gpui::{App, SharedString, Task};
use indoc::formatdoc;
use language::Point;
use language_model::LanguageModelToolResultContent;
use project::WorktreeSettings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::Settings;
use util::paths::PathStyle;

fn planning_tool_content_err(e: impl std::fmt::Display) -> LanguageModelToolResultContent {
    LanguageModelToolResultContent::from(e.to_string())
}

/// Reads the content of the given file in the project.
///
/// - Never attempt to read a path that hasn't been previously mentioned.
/// - For large files, this tool returns a file outline with symbol names and line numbers instead of the full content.
///   This outline IS a successful response - use the line numbers to read specific sections with start_line/end_line.
///   Do NOT retry reading the same file without line numbers if you receive an outline.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlanningReadFileToolInput {
    /// The relative path of the file to read.
    ///
    /// This path should never be absolute, and the first component of the path should always be a root directory in a project.
    ///
    /// <example>
    /// If the project has the following root directories:
    ///
    /// - /a/b/directory1
    /// - /c/d/directory2
    ///
    /// If you want to access `file.txt` in `directory1`, you should use the path `directory1/file.txt`.
    /// If you want to access `file.txt` in `directory2`, you should use the path `directory2/file.txt`.
    /// </example>
    pub path: String,
    /// Optional line number to start reading on (1-based index)
    #[serde(default)]
    pub start_line: Option<u32>,
    /// Optional line number to end reading on (1-based index, inclusive)
    #[serde(default)]
    pub end_line: Option<u32>,
}

/// A simplified read_file tool for planning that doesn't require Thread or ActionLog dependencies.
pub struct PlanningReadFileTool {
    project: Entity<Project>,
}

impl PlanningReadFileTool {
    pub fn new(project: Entity<Project>) -> Self {
        Self { project }
    }
}

impl AgentTool for PlanningReadFileTool {
    type Input = PlanningReadFileToolInput;
    type Output = LanguageModelToolResultContent;

    const NAME: &'static str = "read_file";

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Read
    }

    fn initial_title(
        &self,
        input: Result<Self::Input, serde_json::Value>,
        cx: &mut App,
    ) -> SharedString {
        if let Ok(input) = input
            && let Some(project_path) = self.project.read(cx).find_project_path(&input.path, cx)
            && let Some(path) = self
                .project
                .read(cx)
                .short_full_path_for_project_path(&project_path, cx)
        {
            match (input.start_line, input.end_line) {
                (Some(start), Some(end)) => {
                    format!("Read file `{path}` (lines {}-{})", start, end)
                }
                (Some(start), None) => {
                    format!("Read file `{path}` (from line {})", start)
                }
                _ => format!("Read file `{path}`"),
            }
            .into()
        } else {
            "Read file".into()
        }
    }

    fn run(
        self: Arc<Self>,
        input: Self::Input,
        _event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<LanguageModelToolResultContent, LanguageModelToolResultContent>> {
        let project = self.project.clone();
        cx.spawn(async move |cx| {
            let fs = project.read_with(cx, |project, _cx| project.fs().clone());
            let canonical_roots = canonicalize_worktree_roots(&project, &fs, cx).await;

            let project_path = project
                .read_with(cx, |project, cx| {
                    let resolved =
                        resolve_project_path(project, &input.path, &canonical_roots, cx)?;
                    anyhow::Ok(match resolved {
                        ResolvedProjectPath::Safe(path) => path,
                        ResolvedProjectPath::SymlinkEscape { project_path, .. } => {
                            // For planning, we don't support symlink escapes (no authorization UI)
                            anyhow::bail!(
                                "Cannot read symlink that points outside the project: {}",
                                project_path.path.display(PathStyle::local())
                            );
                        }
                    })
                })
                .map_err(planning_tool_content_err)?;

            let abs_path = project
                .read_with(cx, |project, cx| project.absolute_path(&project_path, cx))
                .ok_or_else(|| anyhow!("Failed to convert {} to absolute path", &input.path))
                .map_err(planning_tool_content_err)?;

            // Check settings exclusions synchronously
            project
                .read_with(cx, |_project, cx| {
                    let global_settings = WorktreeSettings::get_global(cx);
                    if global_settings.is_path_excluded(&project_path.path) {
                        anyhow::bail!(
                            "Cannot read file because its path matches the global `file_scan_exclusions` setting: {}",
                            &input.path
                        );
                    }

                    if global_settings.is_path_private(&project_path.path) {
                        anyhow::bail!(
                            "Cannot read file because its path matches the global `private_files` setting: {}",
                            &input.path
                        );
                    }

                    let worktree_settings =
                        WorktreeSettings::get(Some((&project_path).into()), cx);
                    if worktree_settings.is_path_excluded(&project_path.path) {
                        anyhow::bail!(
                            "Cannot read file because its path matches the worktree `file_scan_exclusions` setting: {}",
                            &input.path
                        );
                    }

                    if worktree_settings.is_path_private(&project_path.path) {
                        anyhow::bail!(
                            "Cannot read file because its path matches the worktree `private_files` setting: {}",
                            &input.path
                        );
                    }

                    anyhow::Ok(())
                })
                .map_err(planning_tool_content_err)?;

            let file_path = input.path.clone();

            let open_buffer_task =
                project.update(cx, |project, cx| project.open_buffer(project_path.clone(), cx));

            let buffer = open_buffer_task.await.map_err(planning_tool_content_err)?;

            if buffer.read_with(cx, |buffer, _| {
                buffer
                    .file()
                    .as_ref()
                    .is_none_or(|file| !file.disk_state().exists())
            }) {
                return Err(planning_tool_content_err(format!("{file_path} not found")));
            }

            // Check if specific line ranges are provided
            if input.start_line.is_some() || input.end_line.is_some() {
                let result = buffer.read_with(cx, |buffer, _cx| {
                    let start = input.start_line.unwrap_or(1).max(1);
                    let start_row = start - 1;
                    let mut end_row = input.end_line.unwrap_or(u32::MAX);
                    if end_row <= start_row {
                        end_row = start_row + 1;
                    }
                    let start = buffer.anchor_before(Point::new(start_row, 0));
                    let end = buffer.anchor_before(Point::new(end_row, 0));
                    buffer.text_for_range(start..end).collect::<String>()
                });

                Ok(result.into())
            } else {
                // No line ranges specified, check file size
                let buffer_content = outline::get_buffer_content_or_outline(
                    buffer.clone(),
                    Some(&abs_path.to_string_lossy()),
                    cx,
                )
                .await
                .map_err(planning_tool_content_err)?;

                if buffer_content.is_outline {
                    Ok(formatdoc! {"
                        SUCCESS: File outline retrieved. This file is too large to read all at once, so the outline below shows the file's structure with line numbers.

                        IMPORTANT: Do NOT retry this call without line numbers - you will get the same outline.
                        Instead, use the line numbers below to read specific sections by calling this tool again with start_line and end_line parameters.

                        {}

                        NEXT STEPS: To read a specific symbol's implementation, call read_file with the same path plus start_line and end_line from the outline above.
                        For example, to read a function shown as [L100-150], use start_line: 100 and end_line: 150.", buffer_content.text
                    }
                    .into())
                } else {
                    Ok(buffer_content.text.into())
                }
            }
        })
    }
}
