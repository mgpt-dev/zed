use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanId(pub Uuid);

impl PlanId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PlanId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a node within a plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Version number for a plan (increments with each change)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlanVersion(pub u64);

impl PlanVersion {
    pub fn initial() -> Self {
        Self(1)
    }

    pub fn increment(&self) -> Self {
        Self(self.0 + 1)
    }
}

/// Metadata about the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanMetadata {
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub template_name: Option<String>,
}

/// Metadata about a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub priority: Option<u8>,
}

/// Type of node in the plan hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Top-level goal or objective
    Goal,
    /// Major phase or milestone
    Phase,
    /// Specific task or action item
    Task,
    /// Constraint or limitation
    Constraint,
    /// Assumption being made
    Assumption,
    /// Decision that was made
    Decision,
    /// General note or observation
    Note,
}

/// A node in the plan tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub content: String,
    pub children: Vec<PlanNode>,
    pub metadata: NodeMetadata,
}

impl PlanNode {
    pub fn new(node_type: NodeType, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: NodeId::new(),
            node_type,
            content,
            children: Vec::new(),
            metadata: NodeMetadata {
                created_at: now,
                updated_at: now,
                tags: Vec::new(),
                priority: None,
            },
        }
    }

    /// Find a node by ID in this subtree
    pub fn find_node(&self, node_id: NodeId) -> Option<&PlanNode> {
        if self.id == node_id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_node(node_id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by ID and return a mutable reference
    pub fn find_node_mut(&mut self, node_id: NodeId) -> Option<&mut PlanNode> {
        if self.id == node_id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_node_mut(node_id) {
                return Some(found);
            }
        }
        None
    }

    /// Collect all node IDs in this subtree
    pub fn collect_node_ids(&self, ids: &mut Vec<NodeId>) {
        ids.push(self.id);
        for child in &self.children {
            child.collect_node_ids(ids);
        }
    }
}

/// The complete plan structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: PlanId,
    pub version: PlanVersion,
    pub metadata: PlanMetadata,
    pub root: PlanNode,
}

