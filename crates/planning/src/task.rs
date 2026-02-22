use crate::plan::{Plan, PlanNode, NodeType, NodeId};
use serde::{Deserialize, Serialize};

/// A task derived from the plan for LLM consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedTask {
    pub id: String,
    pub source_node_id: NodeId,
    pub content: String,
    pub context_path: Vec<String>,
    pub priority: Option<u8>,
    pub dependencies: Vec<String>,
}

impl DerivedTask {
    /// Convert to markdown list item format
    pub fn to_markdown(&self, indent_level: usize) -> String {
        let indent = "  ".repeat(indent_level);
        let priority_marker = self.priority
            .map(|p| format!(" [P{}]", p))
            .unwrap_or_default();
        
        format!("{}- {}{}", indent, self.content, priority_marker)
    }
}

/// Extract actionable tasks from a plan
pub fn derive_tasks_from_plan(plan: &Plan) -> Vec<DerivedTask> {
    let mut tasks = Vec::new();
    let mut context_path = Vec::new();
    
    extract_tasks_recursive(&plan.root, &mut tasks, &mut context_path);
    
    tasks
}

fn extract_tasks_recursive(
    node: &PlanNode,
    tasks: &mut Vec<DerivedTask>,
    context_path: &mut Vec<String>,
) {
    // Add current node to context path
    context_path.push(node.content.clone());
    
    // If this is a Task node, create a DerivedTask
    if node.node_type == NodeType::Task {
        tasks.push(DerivedTask {
            id: node.id.0.to_string(),
            source_node_id: node.id,
            content: node.content.clone(),
            context_path: context_path.clone(),
            priority: node.metadata.priority,
            dependencies: Vec::new(), // TODO: Extract from relationships
        });
    }
    
    // Recursively process children
    for child in &node.children {
        extract_tasks_recursive(child, tasks, context_path);
    }
    
    // Remove current node from context path
    context_path.pop();
}

/// Convert tasks to markdown format for LLM consumption
pub fn tasks_to_markdown(tasks: &[DerivedTask]) -> String {
    let mut markdown = String::from("# Derived Tasks\n\n");
    
    for task in tasks {
        // Show context path as a comment
        if !task.context_path.is_empty() {
            markdown.push_str(&format!("<!-- Context: {} -->\n", task.context_path.join(" > ")));
        }
        
        markdown.push_str(&task.to_markdown(0));
        markdown.push('\n');
    }
    
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanId, PlanVersion, PlanMetadata};
    use chrono::Utc;

    #[test]
    fn test_derive_tasks() {
        let now = Utc::now();
        let mut root = PlanNode::new(NodeType::Goal, "Complete Project".to_string());
        
        let mut phase1 = PlanNode::new(NodeType::Phase, "Phase 1".to_string());
        let task1 = PlanNode::new(NodeType::Task, "Implement feature A".to_string());
        let task2 = PlanNode::new(NodeType::Task, "Write tests".to_string());
        
        phase1.children.push(task1);
        phase1.children.push(task2);
        root.children.push(phase1);
        
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
        
        let tasks = derive_tasks_from_plan(&plan);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].content, "Implement feature A");
        assert_eq!(tasks[1].content, "Write tests");
    }
    
    #[test]
    fn test_tasks_to_markdown() {
        let task = DerivedTask {
            id: "test-id".to_string(),
            source_node_id: NodeId::new(),
            content: "Test task".to_string(),
            context_path: vec!["Goal".to_string(), "Phase 1".to_string()],
            priority: Some(1),
            dependencies: Vec::new(),
        };
        
        let markdown = tasks_to_markdown(&[task]);
        assert!(markdown.contains("# Derived Tasks"));
        assert!(markdown.contains("Test task"));
        assert!(markdown.contains("[P1]"));
    }
}

