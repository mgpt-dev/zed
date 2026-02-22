mod actions;
mod planning_panel;
#[cfg(test)]
mod planning_panel_tests;

pub use actions::*;
pub use planning_panel::PlanningPanel;

use gpui::App;
use workspace::Workspace;

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
            workspace.toggle_panel_focus::<PlanningPanel>(window, cx);
        });
    })
    .detach();
}

