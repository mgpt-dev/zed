# Planning Tool - Integration Guide

## Step 1: Add to Workspace Build

### 1.1 Add to Root Cargo.toml

Add the new crates to the workspace members in `/Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/planning",
    "crates/planning_panel",
]
```

### 1.2 Add Dependencies to Zed Crate

In `crates/zed/Cargo.toml`, add:

```toml
[dependencies]
# ... existing dependencies ...
planning_panel = { path = "../planning_panel" }
```

## Step 2: Initialize in Zed

### 2.1 Initialize Planning Panel

In `crates/zed/src/main.rs` or the appropriate initialization file:

```rust
use planning_panel;

fn main() {
    // ... existing initialization ...
    
    planning_panel::init(&mut cx);
    
    // ... rest of initialization ...
}
```

### 2.2 Register Panel in Workspace

In `crates/workspace/src/workspace.rs`, add the panel registration:

```rust
use planning_panel::PlanningPanel;

impl Workspace {
    pub fn new(...) -> Self {
        // ... existing code ...
        
        // Register planning panel
        cx.observe_new::<PlanningPanel>(|workspace, panel, cx| {
            workspace.add_panel(panel, cx);
        }).detach();
        
        // ... rest of initialization ...
    }
}
```

## Step 3: Add Keybindings

### 3.1 Create Keybindings File

Create or update `assets/keymaps/default.json`:

```json
{
  "context": "Workspace",
  "bindings": {
    "cmd-shift-p": "planning_panel::ToggleFocus",
    "cmd-k cmd-n": "planning_panel::NewPlan",
    "cmd-k cmd-s": "planning_panel::SavePlan",
    "cmd-k cmd-e": "planning_panel::ExportTasks"
  }
}
```

### 3.2 Context-Specific Bindings

```json
{
  "context": "PlanningPanel",
  "bindings": {
    "cmd-n": "planning_panel::AddNode",
    "backspace": "planning_panel::DeleteNode",
    "cmd-enter": "planning_panel::RequestAISuggestion",
    "cmd-y": "planning_panel::ApplySuggestion",
    "cmd-d": "planning_panel::RejectSuggestion"
  }
}
```

## Step 4: Add Menu Items

### 4.1 Update Application Menu

In the appropriate menu configuration file:

```rust
Menu::new("Planning")
    .item("New Plan", planning_panel::NewPlan)
    .item("Save Plan", planning_panel::SavePlan)
    .item("Load Plan", planning_panel::LoadPlan)
    .separator()
    .item("Export Tasks", planning_panel::ExportTasks)
    .separator()
    .item("Toggle Planning Panel", planning_panel::ToggleFocus)
```

## Step 5: Build and Test

### 5.1 Build the Project

```bash
cargo build --release
```

### 5.2 Run Tests

```bash
# Test planning crate
cargo test -p planning

# Test planning_panel crate
cargo test -p planning_panel
```

### 5.3 Run Zed

```bash
cargo run
```

### 5.4 Verify Integration

1. Open Zed
2. Press `Cmd+Shift+P` to toggle planning panel
3. Verify panel appears in right dock
4. Check that panel can be resized and repositioned

## Step 6: Settings Integration (Phase 5)

### 6.1 Create Settings Schema

Create `crates/planning_panel/src/settings.rs`:

```rust
use serde::{Deserialize, Serialize};
use settings::{Settings, SettingsStore};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanningPanelSettings {
    pub default_template: Option<String>,
    pub auto_save: bool,
    pub show_derived_tasks: bool,
    pub ai_suggestions_enabled: bool,
}

impl Default for PlanningPanelSettings {
    fn default() -> Self {
        Self {
            default_template: None,
            auto_save: true,
            show_derived_tasks: true,
            ai_suggestions_enabled: true,
        }
    }
}

impl Settings for PlanningPanelSettings {
    const KEY: Option<&'static str> = Some("planning_panel");
    
    type FileContent = Self;
    
    fn load(
        default_value: &Self::FileContent,
        user_values: &[&Self::FileContent],
        _cx: &mut gpui::App,
    ) -> anyhow::Result<Self> {
        // Merge settings
        Ok(user_values.last().unwrap_or(default_value).clone())
    }
}
```

### 6.2 Register Settings

In `planning_panel::init()`:

```rust
pub fn init(cx: &mut App) {
    actions::init(cx);
    settings::register::<PlanningPanelSettings>(cx);
}
```

## Step 7: Database Schema (if needed)

If you need persistent storage beyond KEY_VALUE_STORE:

### 7.1 Create Migration

In `crates/db/src/migrations/`:

```sql
CREATE TABLE IF NOT EXISTS plans (
    id TEXT PRIMARY KEY,
    workspace_id INTEGER,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_plans_workspace ON plans(workspace_id);
```

## Troubleshooting

### Panel Not Appearing

1. Check that `planning_panel::init()` is called
2. Verify panel is registered in workspace
3. Check keybindings are loaded
4. Look for errors in console

### Build Errors

1. Ensure all dependencies are in Cargo.toml
2. Run `cargo clean` and rebuild
3. Check for version conflicts
4. Verify workspace member paths

### Runtime Errors

1. Check database initialization
2. Verify settings are registered
3. Look for serialization errors
4. Check panel persistence logic

## Development Tips

### Hot Reloading

For faster development iteration:

```bash
cargo watch -x 'run --release'
```

### Debugging

Enable debug logging:

```rust
log::debug!("Planning panel state: {:?}", self.state);
```

### Performance Profiling

Use Zed's built-in profiler:

```rust
use gpui::profile;

profile("render_plan_tree", || {
    // Your rendering code
});
```

## Next Steps After Integration

1. **Phase 2**: Implement tree view and node editing
2. **Phase 3**: Add task derivation UI
3. **Phase 4**: Integrate AI suggestions
4. **Phase 5**: Polish and documentation

## Resources

- Zed Panel Documentation: `crates/workspace/src/dock.rs`
- GPUI Examples: `crates/gpui/examples/`
- Existing Panels: `crates/outline_panel/`, `crates/project_panel/`
- Language Model Integration: `crates/language_model/`

