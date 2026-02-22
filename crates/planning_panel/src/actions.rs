use gpui::actions;

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

