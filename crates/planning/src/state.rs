use crate::plan::{Plan, PlanNode, NodeId, PlanMetadata};
use crate::task::DerivedTask;
use crate::validation::{ValidationError, validate_plan_integrity};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// Events that can modify a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanEvent {
    PlanCreated {
        plan: Plan,
        timestamp: DateTime<Utc>,
    },
    PlanClosed {
        timestamp: DateTime<Utc>,
    },
    NodeAdded {
        parent_id: NodeId,
        node: PlanNode,
        timestamp: DateTime<Utc>,
    },
    NodeUpdated {
        node_id: NodeId,
        new_content: String,
        timestamp: DateTime<Utc>,
    },
    NodeDeleted {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    NodeMoved {
        node_id: NodeId,
        new_parent_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    MetadataUpdated {
        metadata: PlanMetadata,
        timestamp: DateTime<Utc>,
    },
}

/// AI-generated suggestion that requires user approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISuggestion {
    pub id: String,
    pub suggestion_type: SuggestionType,
    pub description: String,
    pub events: Vec<PlanEvent>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    AddNode,
    UpdateNode,
    DeleteNode,
    Restructure,
    Critique,
}

/// The complete state of the planning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningState {
    pub current_plan: Option<Plan>,
    pub history: Vec<PlanEvent>,
    pub derived_tasks: Vec<DerivedTask>,
    pub pending_suggestions: Vec<AISuggestion>,
}

impl PlanningState {
    pub fn new() -> Self {
        Self {
            current_plan: None,
            history: Vec::new(),
            derived_tasks: Vec::new(),
            pending_suggestions: Vec::new(),
        }
    }

    /// Apply an event to the current state
    pub fn apply_event(&mut self, event: PlanEvent) -> Result<(), ValidationError> {
        // Handle PlanCreated specially - it doesn't require an existing plan
        if let PlanEvent::PlanCreated { plan: new_plan, .. } = &event {
            self.current_plan = Some(new_plan.clone());
            self.history.push(event);
            return Ok(());
        }

        // Handle PlanClosed - clears the current plan
        if let PlanEvent::PlanClosed { .. } = &event {
            self.current_plan = None;
            self.history.push(event);
            return Ok(());
        }

        // All other events require an existing plan
        let mut plan = self.current_plan.clone()
            .ok_or(ValidationError::NoPlan)?;

        match &event {
            PlanEvent::PlanCreated { .. } | PlanEvent::PlanClosed { .. } => {
                // Already handled above
                unreachable!()
            }
            PlanEvent::NodeAdded { parent_id, node, .. } => {
                let parent = plan.root.find_node_mut(*parent_id)
                    .ok_or(ValidationError::NodeNotFound(*parent_id))?;
                parent.children.push(node.clone());
                plan.version = plan.version.increment();
                plan.metadata.updated_at = Utc::now();
            }
            PlanEvent::NodeUpdated { node_id, new_content, .. } => {
                let node = plan.root.find_node_mut(*node_id)
                    .ok_or(ValidationError::NodeNotFound(*node_id))?;
                node.content = new_content.clone();
                node.metadata.updated_at = Utc::now();
                plan.version = plan.version.increment();
                plan.metadata.updated_at = Utc::now();
            }
            PlanEvent::NodeDeleted { node_id, .. } => {
                // Find and remove the node
                self.remove_node_from_plan(&mut plan, *node_id)?;
                plan.version = plan.version.increment();
                plan.metadata.updated_at = Utc::now();
            }
            PlanEvent::NodeMoved { node_id, new_parent_id, .. } => {
                // This is complex - need to remove from old parent and add to new
                let node = self.extract_node_from_plan(&mut plan, *node_id)?;
                let new_parent = plan.root.find_node_mut(*new_parent_id)
                    .ok_or(ValidationError::NodeNotFound(*new_parent_id))?;
                new_parent.children.push(node);
                plan.version = plan.version.increment();
                plan.metadata.updated_at = Utc::now();
            }
            PlanEvent::MetadataUpdated { metadata, .. } => {
                plan.metadata = metadata.clone();
                plan.version = plan.version.increment();
            }
        }

        // Validate the plan after applying the event
        validate_plan_integrity(&plan)?;

        self.current_plan = Some(plan);
        self.history.push(event);
        Ok(())
    }

    /// Compute integrity hash for the current plan
    pub fn compute_integrity_hash(&self) -> Option<String> {
        let plan = self.current_plan.as_ref()?;
        let serialized = serde_json::to_string(plan).ok()?;
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        Some(hex::encode(hasher.finalize()))
    }

    fn remove_node_from_plan(&self, plan: &mut Plan, node_id: NodeId) -> Result<(), ValidationError> {
        Self::remove_node_recursive(&mut plan.root, node_id)
    }

    fn remove_node_recursive(node: &mut PlanNode, target_id: NodeId) -> Result<(), ValidationError> {
        node.children.retain(|child| child.id != target_id);
        for child in &mut node.children {
            Self::remove_node_recursive(child, target_id)?;
        }
        Ok(())
    }

    fn extract_node_from_plan(&self, plan: &mut Plan, node_id: NodeId) -> Result<PlanNode, ValidationError> {
        Self::extract_node_recursive(&mut plan.root, node_id)
            .ok_or(ValidationError::NodeNotFound(node_id))
    }

    fn extract_node_recursive(node: &mut PlanNode, target_id: NodeId) -> Option<PlanNode> {
        for i in 0..node.children.len() {
            if node.children[i].id == target_id {
                return Some(node.children.remove(i));
            }
        }
        for child in &mut node.children {
            if let Some(extracted) = Self::extract_node_recursive(child, target_id) {
                return Some(extracted);
            }
        }
        None
    }
}

impl Default for PlanningState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Plan, PlanId, PlanMetadata, PlanNode, PlanVersion, NodeType};

    fn create_test_plan() -> Plan {
        let now = Utc::now();
        Plan {
            id: PlanId::new(),
            version: PlanVersion::initial(),
            metadata: PlanMetadata {
                title: "Test Plan".to_string(),
                description: "A test plan".to_string(),
                created_at: now,
                updated_at: now,
                template_name: None,
            },
            root: PlanNode::new(NodeType::Goal, "Root Goal".to_string()),
        }
    }

    #[test]
    fn test_create_plan_from_empty_state() {
        let mut state = PlanningState::new();
        assert!(state.current_plan.is_none());

        let plan = create_test_plan();
        let event = PlanEvent::PlanCreated {
            plan: plan.clone(),
            timestamp: Utc::now(),
        };

        let result = state.apply_event(event);
        assert!(result.is_ok(), "PlanCreated should succeed on empty state");
        assert!(state.current_plan.is_some());
        assert_eq!(state.current_plan.as_ref().unwrap().metadata.title, "Test Plan");
    }

    #[test]
    fn test_add_node_requires_existing_plan() {
        let mut state = PlanningState::new();

        let event = PlanEvent::NodeAdded {
            parent_id: NodeId::new(),
            node: PlanNode::new(NodeType::Task, "New Task".to_string()),
            timestamp: Utc::now(),
        };

        let result = state.apply_event(event);
        assert!(result.is_err(), "NodeAdded should fail without a plan");
    }

    #[test]
    fn test_add_node_to_existing_plan() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();
        let root_id = plan.root.id;

        // First create the plan
        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();

        // Then add a node
        let new_node = PlanNode::new(NodeType::Task, "New Task".to_string());
        let event = PlanEvent::NodeAdded {
            parent_id: root_id,
            node: new_node.clone(),
            timestamp: Utc::now(),
        };

        let result = state.apply_event(event);
        assert!(result.is_ok(), "NodeAdded should succeed with existing plan");

        let plan = state.current_plan.as_ref().unwrap();
        assert_eq!(plan.root.children.len(), 1);
        assert_eq!(plan.root.children[0].content, "New Task");
    }

    #[test]
    fn test_update_node_content() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();
        let root_id = plan.root.id;

        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();

        let event = PlanEvent::NodeUpdated {
            node_id: root_id,
            new_content: "Updated Goal".to_string(),
            timestamp: Utc::now(),
        };

        let result = state.apply_event(event);
        assert!(result.is_ok());
        assert_eq!(state.current_plan.as_ref().unwrap().root.content, "Updated Goal");
    }

    #[test]
    fn test_event_history() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();
        let root_id = plan.root.id;

        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();

        state.apply_event(PlanEvent::NodeAdded {
            parent_id: root_id,
            node: PlanNode::new(NodeType::Task, "Task 1".to_string()),
            timestamp: Utc::now(),
        }).unwrap();

        state.apply_event(PlanEvent::NodeAdded {
            parent_id: root_id,
            node: PlanNode::new(NodeType::Task, "Task 2".to_string()),
            timestamp: Utc::now(),
        }).unwrap();

        assert_eq!(state.history.len(), 3);
    }

    #[test]
    fn test_close_plan() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();

        // Create a plan
        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();
        assert!(state.current_plan.is_some());

        // Close the plan
        let result = state.apply_event(PlanEvent::PlanClosed {
            timestamp: Utc::now(),
        });
        assert!(result.is_ok());
        assert!(state.current_plan.is_none(), "Plan should be None after closing");
        assert_eq!(state.history.len(), 2);
    }

    #[test]
    fn test_close_plan_without_existing_plan_still_works() {
        let mut state = PlanningState::new();
        assert!(state.current_plan.is_none());

        // Closing without a plan should succeed (idempotent)
        let result = state.apply_event(PlanEvent::PlanClosed {
            timestamp: Utc::now(),
        });
        assert!(result.is_ok());
        assert!(state.current_plan.is_none());
    }

    #[test]
    fn test_update_metadata() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();

        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();

        let now = Utc::now();
        let new_metadata = PlanMetadata {
            title: "Updated Title".to_string(),
            description: "Updated description".to_string(),
            created_at: now,
            updated_at: now,
            template_name: Some("custom".to_string()),
        };

        let result = state.apply_event(PlanEvent::MetadataUpdated {
            metadata: new_metadata.clone(),
            timestamp: now,
        });
        assert!(result.is_ok());
        assert_eq!(state.current_plan.as_ref().unwrap().metadata.title, "Updated Title");
        assert_eq!(state.current_plan.as_ref().unwrap().metadata.description, "Updated description");
    }

    #[test]
    fn test_update_title_only() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();
        let original_description = plan.metadata.description.clone();
        let original_created_at = plan.metadata.created_at;

        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();

        // Update only the title, preserving other metadata
        let mut new_metadata = state.current_plan.as_ref().unwrap().metadata.clone();
        new_metadata.title = "New Title".to_string();
        new_metadata.updated_at = Utc::now();

        let result = state.apply_event(PlanEvent::MetadataUpdated {
            metadata: new_metadata,
            timestamp: Utc::now(),
        });

        assert!(result.is_ok());
        let plan = state.current_plan.as_ref().unwrap();
        assert_eq!(plan.metadata.title, "New Title");
        assert_eq!(plan.metadata.description, original_description);
        assert_eq!(plan.metadata.created_at, original_created_at);
    }

    #[test]
    fn test_metadata_update_increments_version() {
        let mut state = PlanningState::new();
        let plan = create_test_plan();

        state.apply_event(PlanEvent::PlanCreated {
            plan,
            timestamp: Utc::now(),
        }).unwrap();

        let initial_version = state.current_plan.as_ref().unwrap().version;

        // Update metadata
        let mut new_metadata = state.current_plan.as_ref().unwrap().metadata.clone();
        new_metadata.title = "New Title".to_string();

        state.apply_event(PlanEvent::MetadataUpdated {
            metadata: new_metadata,
            timestamp: Utc::now(),
        }).unwrap();

        let new_version = state.current_plan.as_ref().unwrap().version;
        assert!(new_version.0 > initial_version.0, "Version should be incremented after metadata update");
    }

    #[test]
    fn test_metadata_update_without_plan_fails() {
        let mut state = PlanningState::new();

        let now = Utc::now();
        let metadata = PlanMetadata {
            title: "Title".to_string(),
            description: "Desc".to_string(),
            created_at: now,
            updated_at: now,
            template_name: None,
        };

        let result = state.apply_event(PlanEvent::MetadataUpdated {
            metadata,
            timestamp: now,
        });

        assert!(result.is_err(), "MetadataUpdated should fail without an existing plan");
    }
}
