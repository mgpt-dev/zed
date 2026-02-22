mod actions;
mod planning_panel;

pub use actions::*;
pub use planning_panel::PlanningPanel;

use agent_ui::{AgentPanel, InlineAssistant};
use gpui::{App, UpdateGlobal};
use workspace::Workspace;

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
            workspace.toggle_panel_focus::<PlanningPanel>(window, cx);
        });

        workspace.register_action(|workspace, action: &InlineAssist, window, cx| {
            // Get the PlanningPanel
            let Some(planning_panel) = workspace.panel::<PlanningPanel>(cx) else {
                return;
            };

            // Get the markdown editor from the planning panel
            let markdown_editor = planning_panel.read(cx).markdown_editor().clone();

            // Get AgentPanel to access thread_store, prompt_store, and history
            let Some(agent_panel) = workspace.panel::<AgentPanel>(cx) else {
                return;
            };
            let agent_panel = agent_panel.read(cx);

            let thread_store = agent_panel.thread_store().clone();
            let prompt_store = agent_panel.prompt_store().as_ref().cloned();
            let history = agent_panel.history().downgrade();

            let workspace_handle = cx.entity().downgrade();
            let project = workspace.project().downgrade();
            let initial_prompt = action.prompt.clone();

            // Call the InlineAssistant to assist on the markdown editor
            InlineAssistant::update_global(cx, |assistant, cx| {
                assistant.assist(
                    &markdown_editor,
                    workspace_handle,
                    project,
                    thread_store,
                    prompt_store,
                    history,
                    initial_prompt,
                    window,
                    cx,
                );
            });
        });
    })
    .detach();
}

