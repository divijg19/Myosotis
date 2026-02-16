use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NodeId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Ref(NodeId),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub ty: String,
    pub fields: HashMap<String, Value>,
}
