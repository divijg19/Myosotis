use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: u64,
    pub message: Option<String>,
    pub parent: Option<u64>,
    pub changes: HashMap<NodeId, crate::node::Node>,
}
