use crate::commit::Commit;
use crate::node::{Node, NodeId, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub nodes: HashMap<NodeId, Node>,
    pub commits: Vec<Commit>,
    pub next_node_id: NodeId,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            commits: Vec::new(),
            next_node_id: 1,
        }
    }

    pub fn create(&mut self, ty: &str) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let node = Node {
            id,
            ty: ty.to_string(),
            fields: HashMap::new(),
        };

        self.nodes.insert(id, node);
        id
    }

    pub fn set(&mut self, id: NodeId, key: &str, value: Value) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.fields.insert(key.to_string(), value);
        }
    }

    pub fn commit(&mut self, message: Option<String>) {
        let commit_id = self.commits.len() as u64 + 1;

        let snapshot = self.nodes.clone();

        let commit = Commit {
            id: commit_id,
            message,
            parent: self.commits.last().map(|c| c.id),
            changes: snapshot,
        };

        self.commits.push(commit);
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
