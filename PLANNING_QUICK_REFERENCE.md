# Planning Tool - Quick Reference

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Zed Workspace                           │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Planning Panel (UI)                      │  │
│  │  - Tree view rendering                                │  │
│  │  - Node editing                                       │  │
│  │  - Template selection                                 │  │
│  │  - AI suggestion review                               │  │
│  └───────────────────────────────────────────────────────┘  │
│                          │                                   │
│                          ▼                                   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │         Planning State (Event Sourcing)               │  │
│  │  - Current plan                                       │  │
│  │  - Event history                                      │  │
│  │  - Pending AI suggestions                             │  │
│  │  - Derived tasks                                      │  │
│  └───────────────────────────────────────────────────────┘  │
│                          │                                   │
│         ┌────────────────┼────────────────┐                 │
│         ▼                ▼                ▼                 │
│  ┌──────────┐    ┌──────────┐    ┌──────────────┐          │
│  │Templates │    │Validation│    │Task Derivation│          │
│  │Registry  │    │Engine    │    │& Export       │          │
│  └──────────┘    └──────────┘    └──────────────┘          │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │         Language Model Integration                    │  │
│  │  - Suggestion generation                              │  │
│  │  - Plan critique                                      │  │
│  │  - Task breakdown                                     │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Core Data Structures

### Plan Hierarchy
```
Plan
├── id: PlanId (UUID)
├── version: PlanVersion (u64)
├── metadata: PlanMetadata
│   ├── title: String
│   ├── description: String
│   ├── created_at: DateTime<Utc>
│   ├── updated_at: DateTime<Utc>
│   └── template_name: Option<String>
└── root: PlanNode
    ├── id: NodeId (UUID)
    ├── node_type: NodeType
    ├── content: String
    ├── children: Vec<PlanNode>
    └── metadata: NodeMetadata
```

### Node Types
- **Goal** - Top-level objective
- **Phase** - Major milestone
- **Task** - Actionable item
- **Constraint** - Limitation
- **Assumption** - Assumption made
- **Decision** - Decision recorded
- **Note** - General observation

### Event Types
```rust
enum PlanEvent {
    PlanCreated { plan, timestamp },
    NodeAdded { parent_id, node, timestamp },
    NodeUpdated { node_id, new_content, timestamp },
    NodeDeleted { node_id, timestamp },
    NodeMoved { node_id, new_parent_id, timestamp },
    MetadataUpdated { metadata, timestamp },
}
```

## Key APIs

### Creating a Plan from Template
```rust
let registry = TemplateRegistry::new();
let template = registry.get_template("Bug Fix").unwrap();
let plan = template.instantiate(
    "Fix login bug".to_string(),
    "Users cannot log in with SSO".to_string()
);
```

### Applying Events
```rust
let mut state = PlanningState::new();

// Create plan
state.apply_event(PlanEvent::PlanCreated {
    plan: plan.clone(),
    timestamp: Utc::now(),
})?;

// Add node
state.apply_event(PlanEvent::NodeAdded {
    parent_id: parent_node_id,
    node: new_node,
    timestamp: Utc::now(),
})?;
```

### Deriving Tasks
```rust
let tasks = derive_tasks_from_plan(&plan);
let markdown = tasks_to_markdown(&tasks);
// Copy markdown to clipboard or pass to LLM
```

### AI Integration (Phase 4)
```rust
use language_model::{LanguageModelRegistry, LanguageModelRequest};

let model = LanguageModelRegistry::read_global(cx)
    .default_model()
    .map(|configured| configured.model);

let request = LanguageModelRequest {
    messages: vec![
        LanguageModelRequestMessage {
            role: Role::System,
            content: vec!["You are a planning assistant...".into()],
            cache: false,
            reasoning_details: None,
        },
    ],
    // ... other fields
};

let stream = model.stream_completion_text(request, cx).await?;
```

## File Organization

```
crates/
├── planning/                    # Core data model (Phase 1 ✅)
│   ├── src/
│   │   ├── lib.rs              # Public API
│   │   ├── plan.rs             # Plan, PlanNode, NodeType
│   │   ├── state.rs            # PlanningState, PlanEvent
│   │   ├── validation.rs       # Integrity checks
│   │   ├── task.rs             # Task derivation
│   │   └── templates.rs        # Template system
│   └── Cargo.toml
│
└── planning_panel/              # UI panel (Phase 1 ✅, Phase 2-5 pending)
    ├── src/
    │   ├── lib.rs              # Public API
    │   ├── planning_panel.rs   # Main panel implementation
    │   ├── actions.rs          # Action definitions
    │   ├── tree_view.rs        # Tree rendering (TODO)
    │   ├── node_editor.rs      # Node editing (TODO)
    │   ├── modals.rs           # Template picker, etc. (TODO)
    │   ├── task_view.rs        # Task list view (TODO)
    │   ├── ai_assistant.rs     # AI integration (TODO)
    │   └── suggestion_view.rs  # Suggestion review (TODO)
    └── Cargo.toml
```

## Built-in Templates

1. **Bug Fix**
   - Investigation → Implementation → Verification

2. **PRD**
   - Requirements Gathering → Design

3. **Feature Development**
   - Planning → Implementation → Testing & Deployment

4. **Architecture**
   - Analysis → Design

5. **API**
   - Design → Implementation → Documentation

## Testing Strategy

### Unit Tests
- `validation.rs` - Structural integrity
- `task.rs` - Task extraction and markdown
- `state.rs` - Event application
- `templates.rs` - Template instantiation

### Integration Tests
- Panel rendering
- Event sourcing flow
- AI suggestion workflow
- Export functionality

### Manual Testing
- Large plan performance
- UI responsiveness
- Error recovery
- Accessibility

## Development Workflow

1. **Phase 1** ✅ - Core data model complete
2. **Phase 2** - Build interactive UI
3. **Phase 3** - Implement task derivation UI
4. **Phase 4** - Add AI integration
5. **Phase 5** - Polish and documentation

## Next Immediate Steps

1. Register `planning_panel` in workspace initialization
2. Add panel to Zed's build system (Cargo.toml)
3. Implement tree view rendering
4. Add node editing functionality
5. Test basic plan creation and editing

