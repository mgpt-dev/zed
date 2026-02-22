use crate::plan::{Plan, PlanNode, NodeId};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("No plan exists")]
    NoPlan,
    
    #[error("Node not found: {0:?}")]
    NodeNotFound(NodeId),
    
    #[error("Duplicate node ID found: {0:?}")]
    DuplicateNodeId(NodeId),
    
    #[error("Circular dependency detected involving node: {0:?}")]
    CircularDependency(NodeId),
    
    #[error("Orphaned node detected: {0:?}")]
    OrphanedNode(NodeId),
    
    #[error("Invalid node hierarchy")]
    InvalidHierarchy,
    
    #[error("Integrity hash mismatch")]
    IntegrityHashMismatch,
}

/// Validate the structural integrity of a plan
pub fn validate_plan_integrity(plan: &Plan) -> Result<(), ValidationError> {
    // Check for duplicate node IDs
    let mut seen_ids = HashSet::new();
    validate_unique_ids(&plan.root, &mut seen_ids)?;
    
    // Additional validations can be added here:
    // - Check for circular dependencies
    // - Validate parent-child relationships
    // - Ensure no orphaned nodes
    
    Ok(())
}

fn validate_unique_ids(node: &PlanNode, seen_ids: &mut HashSet<NodeId>) -> Result<(), ValidationError> {
    if !seen_ids.insert(node.id) {
        return Err(ValidationError::DuplicateNodeId(node.id));
    }
    
    for child in &node.children {
        validate_unique_ids(child, seen_ids)?;
    }
    
    Ok(())
}

/// Check if a node move would create a circular dependency
pub fn would_create_cycle(plan: &Plan, node_id: NodeId, new_parent_id: NodeId) -> bool {
    // Check if new_parent_id is a descendant of node_id
    if let Some(node) = plan.root.find_node(node_id) {
        is_descendant(node, new_parent_id)
    } else {
        false
    }
}

fn is_descendant(node: &PlanNode, target_id: NodeId) -> bool {
    for child in &node.children {
        if child.id == target_id || is_descendant(child, target_id) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{NodeType, PlanId, PlanVersion, PlanMetadata};
    use chrono::Utc;

    #[test]
    fn test_validate_unique_ids() {
        let now = Utc::now();
        let root = PlanNode::new(NodeType::Goal, "Root".to_string());
        
        let plan = Plan {
            id: PlanId::new(),
            version: PlanVersion::initial(),
            metadata: PlanMetadata {
                title: "Test Plan".to_string(),
                description: "Test".to_string(),
                created_at: now,
                updated_at: now,
                template_name: None,
            },
            root,
        };
        
        assert!(validate_plan_integrity(&plan).is_ok());
    }
}

