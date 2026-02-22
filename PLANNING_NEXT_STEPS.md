# Planning Tool - Next Steps

## Phase 2: Panel Integration (NOT STARTED)

### Goals
Build out the interactive UI for plan editing and visualization.

### Tasks

#### 2.1 Tree View Rendering
- Implement recursive tree rendering in `planning_panel.rs`
- Use GPUI's `v_flex` and `h_flex` for layout
- Add expand/collapse functionality for nodes
- Show node type icons (Goal, Phase, Task, etc.)
- Implement selection state

#### 2.2 Node Editing UI
- Add inline editing for node content
- Implement context menu for node operations:
  - Add child node
  - Delete node
  - Change node type
  - Set priority
  - Add tags
- Use `ui::ContextMenu` for right-click actions

#### 2.3 Template Selection
- Create template picker modal
- Show template descriptions
- Implement template instantiation flow
- Allow custom title/description input

#### 2.4 Plan Persistence
- Implement save/load actions
- Store plan state in `KEY_VALUE_STORE`
- Add plan list view for loading existing plans
- Implement auto-save on changes

#### 2.5 Export Functionality
- Implement "Export Tasks" action
- Generate markdown from current plan
- Copy to clipboard or save to file
- Show preview before export

### Files to Modify/Create
- `crates/planning_panel/src/planning_panel.rs` - Expand render() method
- `crates/planning_panel/src/tree_view.rs` - New file for tree rendering
- `crates/planning_panel/src/modals.rs` - Template picker, plan loader
- `crates/planning_panel/src/node_editor.rs` - Inline editing component

### Integration Points
- Register panel in workspace initialization
- Add keybindings for panel toggle
- Add menu items for planning actions

---

## Phase 3: Task Derivation & Markdown Export (NOT STARTED)

### Goals
Implement task extraction and markdown generation for LLM consumption.

### Tasks

#### 3.1 Task Extraction UI
- Add "Derive Tasks" button to panel
- Show derived tasks in separate view
- Display context path for each task
- Allow filtering by priority/tags

#### 3.2 Markdown Export
- Implement markdown generation from `task.rs`
- Add export options:
  - Include context paths
  - Filter by node type
  - Include metadata (priority, tags)
- Copy to clipboard functionality

#### 3.3 Task Synchronization
- Update derived tasks when plan changes
- Show which tasks are affected by plan edits
- Highlight stale tasks

### Files to Modify/Create
- `crates/planning_panel/src/task_view.rs` - Task list rendering
- `crates/planning/src/task.rs` - Enhance markdown export options
- `crates/planning_panel/src/export.rs` - Export functionality

---

## Phase 4: AI Integration (NOT STARTED)

### Goals
Integrate with Zed's language model system for AI-assisted planning.

### Tasks

#### 4.1 AI Suggestion Engine
- Implement suggestion request flow
- Build prompts for different suggestion types:
  - "Suggest next steps"
  - "Critique this plan"
  - "Break down this task"
  - "Identify missing constraints"
- Use `LanguageModelRegistry::read_global(cx)` to access models
- Stream responses using `stream_completion_text()`

#### 4.2 Suggestion Review UI
- Display pending suggestions in panel
- Show diff preview for suggested changes
- Implement approve/reject actions
- Allow editing suggestions before applying

#### 4.3 Prompt Engineering
- Create system prompts for planning assistant
- Include plan structure in context
- Add examples of good planning practices
- Ensure AI respects plan-first philosophy

#### 4.4 Suggestion Application
- Convert AI suggestions to `PlanEvent`s
- Apply events through state management
- Maintain event history for undo
- Validate suggestions before applying

### Files to Modify/Create
- `crates/planning_panel/src/ai_assistant.rs` - AI integration logic
- `crates/planning_panel/src/suggestion_view.rs` - Suggestion UI
- `crates/planning/src/prompts.rs` - System prompts and templates
- `crates/planning/src/state.rs` - Add suggestion-to-event conversion

### Key Implementation Details

**Accessing Language Models:**
```rust
use language_model::{LanguageModelRegistry, LanguageModelRequest, LanguageModelRequestMessage, Role};

let model = LanguageModelRegistry::read_global(cx)
    .default_model()
    .map(|configured| configured.model);

let request = LanguageModelRequest {
    thread_id: None,
    prompt_id: None,
    intent: Some(CompletionIntent::Planning),
    messages: vec![
        LanguageModelRequestMessage {
            role: Role::System,
            content: vec!["You are a planning assistant...".into()],
            cache: false,
            reasoning_details: None,
        },
        LanguageModelRequestMessage {
            role: Role::User,
            content: vec![user_prompt.into()],
            cache: false,
            reasoning_details: None,
        },
    ],
    tools: Vec::new(),
    tool_choice: None,
    stop: Vec::new(),
    temperature: None,
    thinking_allowed: false,
    thinking_effort: None,
};

let stream = model.stream_completion_text(request, cx).await?;
```

---

## Phase 5: Templates & Polish (NOT STARTED)

### Goals
Refine templates, add polish, and improve UX.

### Tasks

#### 5.1 Template Enhancements
- Add more templates based on user feedback
- Allow custom template creation
- Implement template import/export
- Add template variables (e.g., ${PROJECT_NAME})

#### 5.2 UI Polish
- Add keyboard shortcuts for all actions
- Implement drag-and-drop for node reordering
- Add visual feedback for operations
- Improve error messaging

#### 5.3 Settings Integration
- Add planning panel settings
- Configure default template
- Set AI model preferences
- Customize export format

#### 5.4 Documentation
- Write user guide
- Add inline help tooltips
- Create example plans
- Document keyboard shortcuts

### Files to Modify/Create
- `crates/planning/src/templates.rs` - Template variables, custom templates
- `crates/planning_panel/src/settings.rs` - Panel settings
- `crates/planning_panel/src/keybindings.rs` - Keyboard shortcuts
- Documentation files

---

## Integration Checklist

### Workspace Integration
- [ ] Register panel in `workspace::init()`
- [ ] Add to default panel layout
- [ ] Implement panel serialization/deserialization
- [ ] Add workspace menu items

### Keybindings
- [ ] Define default keybindings in JSON
- [ ] Document all shortcuts
- [ ] Make keybindings customizable

### Settings
- [ ] Create settings schema
- [ ] Add to settings UI
- [ ] Implement settings validation

### Testing
- [ ] Unit tests for all core logic
- [ ] Integration tests for panel
- [ ] Manual testing checklist
- [ ] Performance testing for large plans

---

## Technical Considerations

### Performance
- Use lazy rendering for large plan trees
- Implement virtualization for long task lists
- Cache derived tasks until plan changes
- Debounce auto-save operations

### Error Handling
- Graceful degradation if AI unavailable
- Validate all user input
- Provide clear error messages
- Implement recovery from corrupted state

### Accessibility
- Keyboard navigation for all features
- Screen reader support
- High contrast mode support
- Focus management

### Future Enhancements (Post-MVP)
- Collaboration features (multi-user editing)
- Plan versioning and branching
- Integration with external task systems
- Plan templates marketplace
- Analytics and insights

