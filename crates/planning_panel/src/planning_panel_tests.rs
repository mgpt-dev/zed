#[cfg(test)]
mod tests {
    use gpui::{px, TestAppContext, VisualTestContext};
    use project::Project;
    use settings::SettingsStore;
    use workspace::{dock::DockPosition, Workspace};

    use crate::PlanningPanel;

    fn init_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
            theme::init(theme::LoadThemes::JustBase, cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    async fn test_planning_panel_creation(cx: &mut TestAppContext) {
        init_test(cx);

        let project = Project::test(project::FakeFs::new(cx.executor()), [], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let panel = workspace.update_in(cx, |workspace, window, cx| {
            let panel = PlanningPanel::new(workspace, window, cx);
            workspace.add_panel(panel.clone(), window, cx);
            panel
        });

        cx.run_until_parked();

        // Verify panel was created with default size
        panel.read_with(cx, |panel, _| {
            assert!(panel.width.is_none(), "Panel should start with no width set");
        });
    }

    #[gpui::test]
    async fn test_planning_panel_resize(cx: &mut TestAppContext) {
        init_test(cx);

        let project = Project::test(project::FakeFs::new(cx.executor()), [], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let panel = workspace.update_in(cx, |workspace, window, cx| {
            let panel = PlanningPanel::new(workspace, window, cx);
            workspace.add_panel(panel.clone(), window, cx);
            workspace.toggle_dock(DockPosition::Right, window, cx);
            panel
        });

        cx.run_until_parked();

        // Test resizing the panel
        workspace.update_in(cx, |workspace, window, cx| {
            let right_dock = workspace.right_dock();
            right_dock.update(cx, |dock, cx| {
                dock.resize_active_panel(Some(px(500.)), window, cx);
            });
        });

        cx.run_until_parked();

        // Verify the size was updated
        panel.read_with(cx, |panel, _| {
            assert_eq!(
                panel.width,
                Some(px(500.)),
                "Panel width should be updated to 500px"
            );
        });
    }

    #[gpui::test]
    async fn test_planning_panel_resize_multiple_times(cx: &mut TestAppContext) {
        init_test(cx);

        let project = Project::test(project::FakeFs::new(cx.executor()), [], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let panel = workspace.update_in(cx, |workspace, window, cx| {
            let panel = PlanningPanel::new(workspace, window, cx);
            workspace.add_panel(panel.clone(), window, cx);
            workspace.toggle_dock(DockPosition::Right, window, cx);
            panel
        });

        cx.run_until_parked();

        // Resize multiple times in succession
        for size in [300., 400., 500., 250.] {
            workspace.update_in(cx, |workspace, window, cx| {
                let right_dock = workspace.right_dock();
                right_dock.update(cx, |dock, cx| {
                    dock.resize_active_panel(Some(px(size)), window, cx);
                });
            });
            cx.run_until_parked();
        }

        // Verify final size
        panel.read_with(cx, |panel, _| {
            assert_eq!(
                panel.width,
                Some(px(250.)),
                "Panel width should be 250px after multiple resizes"
            );
        });
    }

    #[gpui::test]
    async fn test_planning_panel_reset_size(cx: &mut TestAppContext) {
        init_test(cx);

        let project = Project::test(project::FakeFs::new(cx.executor()), [], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let panel = workspace.update_in(cx, |workspace, window, cx| {
            let panel = PlanningPanel::new(workspace, window, cx);
            workspace.add_panel(panel.clone(), window, cx);
            workspace.toggle_dock(DockPosition::Right, window, cx);
            panel
        });

        cx.run_until_parked();

        // Set a size
        workspace.update_in(cx, |workspace, window, cx| {
            let right_dock = workspace.right_dock();
            right_dock.update(cx, |dock, cx| {
                dock.resize_active_panel(Some(px(600.)), window, cx);
            });
        });
        cx.run_until_parked();

        // Reset to default (None)
        workspace.update_in(cx, |workspace, window, cx| {
            let right_dock = workspace.right_dock();
            right_dock.update(cx, |dock, cx| {
                dock.resize_active_panel(None, window, cx);
            });
        });
        cx.run_until_parked();

        // Verify size was reset
        panel.read_with(cx, |panel, _| {
            assert!(panel.width.is_none(), "Panel width should be reset to None");
        });
    }
}

