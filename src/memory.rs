use crate::commit::Commit;
use crate::error::MyosotisError;
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

    pub fn validate(&self) -> Result<(), MyosotisError> {
        // Commit ids must be strictly increasing and linear parent chain
        for (i, commit) in self.commits.iter().enumerate() {
            let expected_id = i as u64 + 1;
            if commit.id != expected_id {
                return Err(MyosotisError::Invariant(format!(
                    "commit id {} at index {} expected {}",
                    commit.id, i, expected_id
                )));
            }

            if i == 0 {
                if commit.parent.is_some() {
                    return Err(MyosotisError::Invariant(
                        "first commit must have no parent".to_string(),
                    ));
                }
            } else {
                let prev_id = expected_id - 1;
                if commit.parent != Some(prev_id) {
                    return Err(MyosotisError::Invariant(format!(
                        "commit {} has invalid parent {:?}, expected {}",
                        commit.id, commit.parent, prev_id
                    )));
                }
            }

            // Commit snapshot internal checks
            for (nid, node) in &commit.changes {
                if nid != &node.id {
                    return Err(MyosotisError::Invariant(format!(
                        "commit {} contains node key {} but node.id {}",
                        commit.id, nid, node.id
                    )));
                }

                if node.id >= self.next_node_id {
                    return Err(MyosotisError::Invariant(format!(
                        "node id {} in commit {} >= next_node_id {}",
                        node.id, commit.id, self.next_node_id
                    )));
                }

                // Check values for invalid references
                fn check_value_refs(
                    v: &Value,
                    snapshot: &HashMap<NodeId, Node>,
                ) -> Result<(), MyosotisError> {
                    match v {
                        Value::Ref(rid) => {
                            if !snapshot.contains_key(rid) {
                                return Err(MyosotisError::Invariant(format!(
                                    "reference to missing node {}",
                                    rid
                                )));
                            }
                        }
                        Value::List(vec) => {
                            for item in vec {
                                check_value_refs(item, snapshot)?;
                            }
                        }
                        Value::Map(map) => {
                            for (_k, item) in map {
                                check_value_refs(item, snapshot)?;
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                }

                for (_k, val) in &node.fields {
                    check_value_refs(val, &commit.changes)?;
                }
            }
        }

        // Current nodes checks
        for (nid, node) in &self.nodes {
            if nid != &node.id {
                return Err(MyosotisError::Invariant(format!(
                    "current nodes key {} does not match node.id {}",
                    nid, node.id
                )));
            }

            if *nid >= self.next_node_id {
                return Err(MyosotisError::Invariant(format!(
                    "current node id {} >= next_node_id {}",
                    nid, self.next_node_id
                )));
            }
        }

        Ok(())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
