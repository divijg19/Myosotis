use crate::node::{NodeId, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Mutation {
    CreateNode {
        id: NodeId,
        ty: String,
    },
    SetField {
        id: NodeId,
        key: String,
        value: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: u64,
    pub parent: Option<u64>,
    pub parent_hash: Option<[u8; 32]>,
    pub hash: [u8; 32],
    pub message: Option<String>,
    pub mutations: Vec<Mutation>,
}
