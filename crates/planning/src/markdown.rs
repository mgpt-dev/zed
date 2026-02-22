//! Markdown parsing and rendering for plans

use crate::plan::{Plan, PlanId, PlanMetadata, PlanNode, PlanVersion, NodeType, NodeId};
use chrono::Utc;

/// Parse markdown content into a Plan
pub fn parse_markdown_to_plan(content: &str) -> Option<Plan> {
    let mut lines = content.lines().peekable();
    let (name, description) = parse_frontmatter(&mut lines)?;
    let root = parse_plan_content(&mut lines);
    
    let now = Utc::now();
    Some(Plan {
        id: PlanId::new(),
        version: PlanVersion::initial(),
        metadata: PlanMetadata {
            title: name,
            description,
            created_at: now,
            updated_at: now,
            template_name: None,
        },
        root,
    })
}

fn parse_frontmatter<'a>(lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>) -> Option<(String, String)> {
    if lines.peek()?.trim() != "---" {
        return Some(("Untitled Plan".to_string(), String::new()));
    }
    lines.next();

    let mut name = String::new();
    let mut description = String::new();

    loop {
        let line = lines.next()?;
        if line.trim() == "---" { break; }
        if let Some(v) = line.trim().strip_prefix("name:") { name = v.trim().to_string(); }
        else if let Some(v) = line.trim().strip_prefix("description:") { description = v.trim().to_string(); }
    }

    Some((if name.is_empty() { "Untitled Plan".to_string() } else { name }, description))
}

fn parse_plan_content<'a>(lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>) -> PlanNode {
    let mut root = PlanNode::new(NodeType::Goal, "Plan".to_string());
    let mut current_h2: Option<NodeId> = None;
    let mut current_h3: Option<NodeId> = None;
    
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        
        if let Some(h) = trimmed.strip_prefix("## ") {
            let content = h.strip_prefix("Goal:").unwrap_or(h).trim();
            let node = PlanNode::new(NodeType::Goal, content.to_string());
            current_h2 = Some(node.id);
            current_h3 = None;
            root.children.push(node);
        } else if let Some(h) = trimmed.strip_prefix("### ") {
            // ### = Phase (sub-section)
            let content = h.strip_prefix("Phase:").unwrap_or(h).trim();
            let node = PlanNode::new(NodeType::Phase, content.to_string());
            let nid = node.id;
            if let Some(pid) = current_h2 {
                if let Some(p) = root.find_node_mut(pid) { p.children.push(node); current_h3 = Some(nid); }
            } else { root.children.push(node); current_h3 = Some(nid); }
        } else if let Some(t) = trimmed.strip_prefix("- [ ] ") {
            add_item(&mut root, current_h3.or(current_h2), NodeType::Task, t.to_string());
        } else if let Some(t) = trimmed.strip_prefix("- [x] ") {
            add_item(&mut root, current_h3.or(current_h2), NodeType::Task, format!("✓ {}", t));
        } else if let Some(n) = trimmed.strip_prefix("- ") {
            add_item(&mut root, current_h3.or(current_h2), NodeType::Note, n.to_string());
        }
    }
    root
}

fn add_item(root: &mut PlanNode, parent_id: Option<NodeId>, nt: NodeType, content: String) {
    let node = PlanNode::new(nt, content);
    if let Some(pid) = parent_id {
        if let Some(p) = root.find_node_mut(pid) { p.children.push(node); return; }
    }
    root.children.push(node);
}

/// Render a Plan to markdown
pub fn render_plan_to_markdown(plan: &Plan) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", plan.metadata.title));
    if !plan.metadata.description.is_empty() {
        out.push_str(&format!("description: {}\n", plan.metadata.description));
    }
    out.push_str("---\n\n");
    for child in &plan.root.children { render_node(child, 0, &mut out); }
    out
}

fn render_node(node: &PlanNode, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth.saturating_sub(1));
    match node.node_type {
        NodeType::Goal => out.push_str(&format!("## Goal: {}\n\n", node.content)),
        NodeType::Phase => out.push_str(&format!("### Phase: {}\n\n", node.content)),
        NodeType::Task => {
            if node.content.starts_with("✓ ") {
                out.push_str(&format!("{}- [x] {}\n", indent, &node.content[4..]));
            } else {
                out.push_str(&format!("{}- [ ] {}\n", indent, node.content));
            }
        }
        NodeType::Note => out.push_str(&format!("{}- {}\n", indent, node.content)),
        NodeType::Decision => out.push_str(&format!("{}- Decision: {}\n", indent, node.content)),
        NodeType::Constraint => out.push_str(&format!("{}- Constraint: {}\n", indent, node.content)),
        NodeType::Assumption => out.push_str(&format!("{}- Assumption: {}\n", indent, node.content)),
    }
    for child in &node.children { render_node(child, depth + 1, out); }
    if matches!(node.node_type, NodeType::Goal | NodeType::Phase) && !node.children.is_empty() { out.push('\n'); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let c = "---\nname: Test Plan\ndescription: A test\n---\n\n## Goal: Do it\n";
        let plan = parse_markdown_to_plan(c).unwrap();
        assert_eq!(plan.metadata.title, "Test Plan");
        assert_eq!(plan.metadata.description, "A test");
    }

    #[test]
    fn test_parse_goals() {
        let c = "---\nname: P\n---\n\n## Goal: Build\n- [ ] Task\n";
        let plan = parse_markdown_to_plan(c).unwrap();
        assert_eq!(plan.root.children.len(), 1);
        assert_eq!(plan.root.children[0].content, "Build");
    }

    #[test]
    fn test_render() {
        let c = "---\nname: Test\n---\n\n## Goal: Main\n\n- [ ] Task\n\n";
        let plan = parse_markdown_to_plan(c).unwrap();
        let r = render_plan_to_markdown(&plan);
        assert!(r.contains("name: Test"));
        assert!(r.contains("## Goal: Main"));
    }

    #[test]
    fn test_parse_empty_name_defaults_to_untitled() {
        let c = "---\nname: \ndescription: A test\n---\n\n## Goal: Do it\n";
        let plan = parse_markdown_to_plan(c).unwrap();
        assert_eq!(plan.metadata.title, "Untitled Plan");
    }

    #[test]
    fn test_parse_missing_frontmatter_defaults_to_untitled() {
        let c = "## Goal: Do it\n- [ ] Task\n";
        let plan = parse_markdown_to_plan(c).unwrap();
        assert_eq!(plan.metadata.title, "Untitled Plan");
    }

    #[test]
    fn test_title_roundtrip() {
        // Parse -> modify title -> render -> parse again should preserve title
        let c = "---\nname: Original Title\ndescription: A test\n---\n\n## Goal: Do it\n";
        let mut plan = parse_markdown_to_plan(c).unwrap();

        // Modify title
        plan.metadata.title = "Updated Title".to_string();

        // Render and parse again
        let rendered = render_plan_to_markdown(&plan);
        let reparsed = parse_markdown_to_plan(&rendered).unwrap();

        assert_eq!(reparsed.metadata.title, "Updated Title");
    }

    #[test]
    fn test_title_with_special_characters() {
        let c = "---\nname: My Plan: A Test (2024)\ndescription: Testing\n---\n\n## Goal: Do it\n";
        let plan = parse_markdown_to_plan(c).unwrap();
        assert_eq!(plan.metadata.title, "My Plan: A Test (2024)");

        // Verify roundtrip
        let rendered = render_plan_to_markdown(&plan);
        let reparsed = parse_markdown_to_plan(&rendered).unwrap();
        assert_eq!(reparsed.metadata.title, "My Plan: A Test (2024)");
    }
}

