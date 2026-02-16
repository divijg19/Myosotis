use crate::commit::{Commit, Mutation};
use crate::error::MyosotisError;
use crate::node::{Node, NodeId, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub commits: Vec<Commit>,
    pub next_node_id: NodeId,

    #[serde(skip)]
    pub head_state: HashMap<NodeId, Node>,

    #[serde(skip)]
    pub pending_mutations: Vec<Mutation>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            commits: Vec::new(),
            next_node_id: 1,
            head_state: HashMap::new(),
            pending_mutations: Vec::new(),
        }
    }

    pub fn compute_commit_hash(
        parent_hash: Option<[u8; 32]>,
        message: &Option<String>,
        mutations: &[Mutation],
    ) -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Parent hash: 32 bytes
        match parent_hash {
            Some(ph) => hasher.update(ph),
            None => hasher.update([0u8; 32]),
        }

        // Message: length u64 BE + bytes (empty if None)
        if let Some(msg) = message {
            let len = msg.len() as u64;
            hasher.update(len.to_be_bytes());
            hasher.update(msg.as_bytes());
        } else {
            hasher.update(0u64.to_be_bytes());
        }

        // Mutations in order
        for m in mutations {
            match m {
                Mutation::CreateNode { id, ty } => {
                    hasher.update([0x01u8]);
                    hasher.update(id.to_be_bytes());
                    let tlen = ty.len() as u64;
                    hasher.update(tlen.to_be_bytes());
                    hasher.update(ty.as_bytes());
                }
                Mutation::SetField { id, key, value } => {
                    hasher.update([0x02u8]);
                    hasher.update(id.to_be_bytes());
                    let klen = key.len() as u64;
                    hasher.update(klen.to_be_bytes());
                    hasher.update(key.as_bytes());
                    // serialize value canonically
                    fn write_value(hasher: &mut Sha256, v: &Value) {
                        match v {
                            Value::Int(i) => {
                                hasher.update([0x01u8]);
                                hasher.update(i.to_be_bytes());
                            }
                            Value::Float(f) => {
                                hasher.update([0x02u8]);
                                hasher.update(f.to_bits().to_be_bytes());
                            }
                            Value::Bool(b) => {
                                hasher.update([0x03u8]);
                                hasher.update([*b as u8]);
                            }
                            Value::Str(s) => {
                                hasher.update([0x04u8]);
                                let slen = s.len() as u64;
                                hasher.update(slen.to_be_bytes());
                                hasher.update(s.as_bytes());
                            }
                            Value::Ref(rid) => {
                                hasher.update([0x05u8]);
                                hasher.update(rid.to_be_bytes());
                            }
                            Value::List(vec) => {
                                hasher.update([0x06u8]);
                                let len = vec.len() as u64;
                                hasher.update(len.to_be_bytes());
                                for item in vec {
                                    write_value(hasher, item);
                                }
                            }
                            Value::Map(map) => {
                                hasher.update([0x07u8]);
                                // keys sorted
                                let mut keys: Vec<&String> = map.keys().collect();
                                keys.sort();
                                let len = keys.len() as u64;
                                hasher.update(len.to_be_bytes());
                                for k in keys {
                                    let v = map.get(k).unwrap();
                                    let klen = k.len() as u64;
                                    hasher.update(klen.to_be_bytes());
                                    hasher.update(k.as_bytes());
                                    write_value(hasher, v);
                                }
                            }
                        }
                    }

                    write_value(&mut hasher, value);
                }
            }
        }

        let result = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result[..32]);
        out
    }

    pub fn create(&mut self, ty: &str) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let m = Mutation::CreateNode {
            id,
            ty: ty.to_string(),
        };
        // apply immediately
        self.apply_mutation(&m)
            .expect("apply create should succeed");
        self.pending_mutations.push(m);
        id
    }

    pub fn set(&mut self, id: NodeId, key: &str, value: Value) -> Result<(), MyosotisError> {
        if !self.head_state.contains_key(&id) {
            return Err(MyosotisError::NodeNotFound(id));
        }

        let m = Mutation::SetField {
            id,
            key: key.to_string(),
            value,
        };
        self.apply_mutation(&m)?;
        self.pending_mutations.push(m);
        Ok(())
    }

    pub fn commit(&mut self, message: Option<String>) -> Result<(), MyosotisError> {
        if self.pending_mutations.is_empty() {
            return Err(MyosotisError::InvalidInput(
                "no pending mutations".to_string(),
            ));
        }

        let commit_id = self.commits.len() as u64 + 1;
        let parent = self.commits.last().map(|c| c.id);

        // validate parent
        if let Some(p) = parent {
            if p + 1 != commit_id {
                return Err(MyosotisError::Invariant(format!(
                    "invalid parent {} for commit {}",
                    p, commit_id
                )));
            }
        } else if commit_id != 1 {
            return Err(MyosotisError::Invariant(
                "first commit id must be 1".to_string(),
            ));
        }

        // clone pending as commit mutations
        let mutations = self.pending_mutations.clone();

        // Validate pending mutations against committed state (replay of existing commits)
        let mut base_state = Self::replay(&self.commits)?;
        for m in &mutations {
            match m {
                Mutation::CreateNode { id, ty: _ } => {
                    if base_state.contains_key(id) {
                        return Err(MyosotisError::Invariant(format!(
                            "create node id {} already exists",
                            id
                        )));
                    }
                    // simulate create
                    base_state.insert(
                        *id,
                        Node {
                            id: *id,
                            ty: String::new(),
                            fields: HashMap::new(),
                        },
                    );
                }
                Mutation::SetField { id, key, value } => {
                    if !base_state.contains_key(id) {
                        return Err(MyosotisError::Invariant(format!(
                            "set on missing node {}",
                            id
                        )));
                    }
                    // check references inside value
                    fn check_value(
                        v: &Value,
                        state: &HashMap<NodeId, Node>,
                    ) -> Result<(), MyosotisError> {
                        match v {
                            Value::Ref(rid) => {
                                if !state.contains_key(rid) {
                                    return Err(MyosotisError::Invariant(format!(
                                        "reference to missing node {}",
                                        rid
                                    )));
                                }
                            }
                            Value::List(vec) => {
                                for item in vec {
                                    check_value(item, state)?;
                                }
                            }
                            Value::Map(map) => {
                                for item in map.values() {
                                    check_value(item, state)?;
                                }
                            }
                            _ => {}
                        }
                        Ok(())
                    }

                    check_value(value, &base_state)?;
                    // simulate set
                    if let Some(node) = base_state.get_mut(id) {
                        node.fields.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        // compute parent_hash and commit hash
        let parent_hash = self.commits.last().map(|c| c.hash);
        let hash = Self::compute_commit_hash(parent_hash, &message, &mutations);

        let commit = Commit {
            id: commit_id,
            parent,
            parent_hash,
            hash,
            message,
            mutations,
        };

        self.commits.push(commit);
        self.pending_mutations.clear();
        Ok(())
    }

    fn apply_mutation(&mut self, m: &Mutation) -> Result<(), MyosotisError> {
        match m {
            Mutation::CreateNode { id, ty } => {
                if self.head_state.contains_key(id) {
                    return Err(MyosotisError::Invariant(format!(
                        "create existing id {}",
                        id
                    )));
                }
                let node = Node {
                    id: *id,
                    ty: ty.clone(),
                    fields: HashMap::new(),
                };
                self.head_state.insert(*id, node);
                Ok(())
            }
            Mutation::SetField { id, key, value } => {
                let node = self
                    .head_state
                    .get_mut(id)
                    .ok_or(MyosotisError::NodeNotFound(*id))?;
                node.fields.insert(key.clone(), value.clone());
                Ok(())
            }
        }
    }

    pub fn replay(commits: &[Commit]) -> Result<HashMap<NodeId, Node>, MyosotisError> {
        let mut state: HashMap<NodeId, Node> = HashMap::new();

        for commit in commits {
            // basic commit id/parent checks
            // parent consistency is checked elsewhere (storage/load)

            for m in &commit.mutations {
                match m {
                    Mutation::CreateNode { id, ty } => {
                        if state.contains_key(id) {
                            return Err(MyosotisError::Invariant(format!(
                                "duplicate create node {}",
                                id
                            )));
                        }
                        let node = Node {
                            id: *id,
                            ty: ty.clone(),
                            fields: HashMap::new(),
                        };
                        state.insert(*id, node);
                    }
                    Mutation::SetField { id, key, value } => {
                        // Ensure node exists at this time
                        if !state.contains_key(id) {
                            return Err(MyosotisError::Invariant(format!(
                                "set before create {}",
                                id
                            )));
                        }

                        // Check references inside value point to existing node ids in state
                        fn check_value(
                            v: &Value,
                            state: &HashMap<NodeId, Node>,
                        ) -> Result<(), MyosotisError> {
                            match v {
                                Value::Ref(rid) => {
                                    if !state.contains_key(rid) {
                                        return Err(MyosotisError::Invariant(format!(
                                            "reference to missing node {}",
                                            rid
                                        )));
                                    }
                                }
                                Value::List(vec) => {
                                    for item in vec {
                                        check_value(item, state)?;
                                    }
                                }
                                Value::Map(map) => {
                                    for item in map.values() {
                                        check_value(item, state)?;
                                    }
                                }
                                _ => {}
                            }
                            Ok(())
                        }

                        check_value(value, &state)?;
                        let node = state.get_mut(id).ok_or(MyosotisError::Invariant(format!(
                            "set before create {}",
                            id
                        )))?;
                        node.fields.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        Ok(state)
    }

    pub fn validate(&self) -> Result<(), MyosotisError> {
        // Check commit ids and parent chain
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
        }

        // Replay to ensure mutations valid
        let state = Self::replay(&self.commits)?;

        // Ensure next_node_id is greater than any created id
        let max_id = state.keys().copied().max().unwrap_or(0);
        if self.next_node_id <= max_id {
            return Err(MyosotisError::Invariant(format!(
                "next_node_id {} <= max created id {}",
                self.next_node_id, max_id
            )));
        }

        // head_state, if present, must match replayed state
        if !self.head_state.is_empty() && self.head_state != state {
            return Err(MyosotisError::Invariant(
                "head_state does not match replayed state".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
