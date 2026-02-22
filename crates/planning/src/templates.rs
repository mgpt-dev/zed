use crate::plan::{Plan, PlanNode, NodeType, PlanId, PlanVersion, PlanMetadata};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTemplate {
    pub name: String,
    pub description: String,
    pub root_template: NodeTemplate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTemplate {
    pub node_type: NodeType,
    pub content: String,
    pub children: Vec<NodeTemplate>,
}

impl NodeTemplate {
    fn to_plan_node(&self) -> PlanNode {
        let mut node = PlanNode::new(self.node_type, self.content.clone());
        node.children = self.children.iter().map(|t| t.to_plan_node()).collect();
        node
    }
}

impl PlanTemplate {
    pub fn instantiate(&self, title: String, description: String) -> Plan {
        let now = Utc::now();

        Plan {
            id: PlanId::new(),
            version: PlanVersion::initial(),
            metadata: PlanMetadata {
                title,
                description,
                created_at: now,
                updated_at: now,
                template_name: Some(self.name.clone()),
            },
            root: self.root_template.to_plan_node(),
        }
    }

    /// Generate markdown content for this template
    pub fn to_markdown(&self, title: &str, description: &str) -> String {
        let plan = self.instantiate(title.to_string(), description.to_string());
        crate::markdown::render_plan_to_markdown(&plan)
    }
}

pub struct TemplateRegistry {
    templates: Vec<PlanTemplate>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            templates: Vec::new(),
        };
        
        registry.register_builtin_templates();
        registry
    }
    
    pub fn get_template(&self, name: &str) -> Option<&PlanTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }
    
    pub fn list_templates(&self) -> Vec<&PlanTemplate> {
        self.templates.iter().collect()
    }
    
    fn register_builtin_templates(&mut self) {
        // Bug Fix Template
        self.templates.push(PlanTemplate {
            name: "Bug Fix".to_string(),
            description: "Template for fixing bugs".to_string(),
            root_template: NodeTemplate {
                node_type: NodeType::Goal,
                content: "Fix Bug: [Bug Description]".to_string(),
                children: vec![
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Investigation".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Reproduce the bug".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Identify root cause".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Review related code".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Implementation".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Implement fix".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Add regression tests".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Verification".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Verify fix resolves issue".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Run full test suite".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                ],
            },
        });
        
        // PRD Template
        self.templates.push(PlanTemplate {
            name: "PRD".to_string(),
            description: "Product Requirements Document template".to_string(),
            root_template: NodeTemplate {
                node_type: NodeType::Goal,
                content: "Product: [Product Name]".to_string(),
                children: vec![
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Requirements Gathering".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define user personas".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Identify use cases".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Document functional requirements".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Design".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Create wireframes".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define technical architecture".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                ],
            },
        });

        // Feature Development Template
        self.templates.push(PlanTemplate {
            name: "Feature Development".to_string(),
            description: "Template for developing new features".to_string(),
            root_template: NodeTemplate {
                node_type: NodeType::Goal,
                content: "Feature: [Feature Name]".to_string(),
                children: vec![
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Planning".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define feature scope".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Identify dependencies".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Implementation".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Implement core functionality".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Add UI components".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Write unit tests".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Testing & Deployment".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Integration testing".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Deploy to staging".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                ],
            },
        });

        // Architecture Template
        self.templates.push(PlanTemplate {
            name: "Architecture".to_string(),
            description: "Template for architectural design".to_string(),
            root_template: NodeTemplate {
                node_type: NodeType::Goal,
                content: "Architecture: [System Name]".to_string(),
                children: vec![
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Analysis".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Analyze requirements".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Identify constraints".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Design".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define system components".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Design data models".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define interfaces".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                ],
            },
        });

        // API Template
        self.templates.push(PlanTemplate {
            name: "API".to_string(),
            description: "Template for API development".to_string(),
            root_template: NodeTemplate {
                node_type: NodeType::Goal,
                content: "API: [API Name]".to_string(),
                children: vec![
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Design".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define endpoints".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Design request/response schemas".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Define authentication strategy".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Implementation".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Implement endpoints".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Add validation".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Write API tests".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                    NodeTemplate {
                        node_type: NodeType::Phase,
                        content: "Documentation".to_string(),
                        children: vec![
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Generate OpenAPI spec".to_string(),
                                children: vec![],
                            },
                            NodeTemplate {
                                node_type: NodeType::Task,
                                content: "Write usage examples".to_string(),
                                children: vec![],
                            },
                        ],
                    },
                ],
            },
        });
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

