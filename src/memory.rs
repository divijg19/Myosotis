use crate::commit::{Commit, Mutation};
use crate::error::MyosotisError;
use crate::node::{Node, NodeId, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const CHECKPOINT_INTERVAL: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub commit_id: u64,
    pub commit_hash: [u8; 32],
    pub state_hash: [u8; 32],
    pub state: HashMap<NodeId, Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub genesis_state: Option<HashMap<NodeId, Node>>,
    pub genesis_state_hash: Option<[u8; 32]>,
    pub commits: Vec<Commit>,
    pub checkpoints: Vec<Checkpoint>,
    pub next_node_id: NodeId,

    #[serde(skip)]
    pub head_state: HashMap<NodeId, Node>,

    #[serde(skip)]
    pub pending_mutations: Vec<Mutation>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            genesis_state: None,
            genesis_state_hash: None,
            commits: Vec::new(),
            checkpoints: Vec::new(),
            next_node_id: 1,
            head_state: HashMap::new(),
            pending_mutations: Vec::new(),
        }
    }

    fn write_value_canonical(buf: &mut Vec<u8>, value: &Value) {
        match value {
            Value::Int(v) => {
                buf.push(0x01);
                buf.extend_from_slice(&v.to_be_bytes());
            }
            Value::Float(v) => {
                buf.push(0x02);
                buf.extend_from_slice(&v.to_bits().to_be_bytes());
            }
            Value::Bool(v) => {
                buf.push(0x03);
                buf.push(if *v { 0x01 } else { 0x00 });
            }
            Value::Str(v) => {
                buf.push(0x04);
                let len = v.len() as u64;
                buf.extend_from_slice(&len.to_be_bytes());
                buf.extend_from_slice(v.as_bytes());
            }
            Value::Ref(v) => {
                buf.push(0x05);
                buf.extend_from_slice(&v.to_be_bytes());
            }
            Value::List(values) => {
                buf.push(0x06);
                let len = values.len() as u64;
                buf.extend_from_slice(&len.to_be_bytes());
                for item in values {
                    Self::write_value_canonical(buf, item);
                }
            }
            Value::Map(map) => {
                buf.push(0x07);
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                let len = keys.len() as u64;
                buf.extend_from_slice(&len.to_be_bytes());
                for key in keys {
                    let key_len = key.len() as u64;
                    buf.extend_from_slice(&key_len.to_be_bytes());
                    buf.extend_from_slice(key.as_bytes());
                    if let Some(map_value) = map.get(key) {
                        Self::write_value_canonical(buf, map_value);
                    }
                }
            }
        }
    }

    pub fn compute_commit_hash(
        parent_hash: Option<[u8; 32]>,
        message: &Option<String>,
        mutations: &[Mutation],
    ) -> [u8; 32] {
        let mut bytes = Vec::new();

        match parent_hash {
            Some(ph) => bytes.extend_from_slice(&ph),
            None => bytes.extend_from_slice(&[0u8; 32]),
        }

        if let Some(msg) = message {
            let len = msg.len() as u64;
            bytes.extend_from_slice(&len.to_be_bytes());
            bytes.extend_from_slice(msg.as_bytes());
        } else {
            bytes.extend_from_slice(&0u64.to_be_bytes());
        }

        for m in mutations {
            match m {
                Mutation::CreateNode { id, ty } => {
                    bytes.push(0x01);
                    bytes.extend_from_slice(&id.to_be_bytes());
                    let tlen = ty.len() as u64;
                    bytes.extend_from_slice(&tlen.to_be_bytes());
                    bytes.extend_from_slice(ty.as_bytes());
                }
                Mutation::SetField { id, key, value } => {
                    bytes.push(0x02);
                    bytes.extend_from_slice(&id.to_be_bytes());
                    let klen = key.len() as u64;
                    bytes.extend_from_slice(&klen.to_be_bytes());
                    bytes.extend_from_slice(key.as_bytes());
                    Self::write_value_canonical(&mut bytes, value);
                }
                Mutation::DeleteField { id, key } => {
                    bytes.push(0x03);
                    bytes.extend_from_slice(&id.to_be_bytes());
                    let klen = key.len() as u64;
                    bytes.extend_from_slice(&klen.to_be_bytes());
                    bytes.extend_from_slice(key.as_bytes());
                }
                Mutation::DeleteNode { id } => {
                    bytes.push(0x04);
                    bytes.extend_from_slice(&id.to_be_bytes());
                }
            }
        }

        let digest = Sha256::digest(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    pub fn compute_state_hash(state: &HashMap<NodeId, Node>) -> [u8; 32] {
        let mut bytes = Vec::new();
        let mut node_ids: Vec<NodeId> = state.keys().copied().collect();
        node_ids.sort_unstable();

        for node_id in node_ids {
            if let Some(node) = state.get(&node_id) {
                bytes.extend_from_slice(&node_id.to_be_bytes());

                let ty_len = node.ty.len() as u64;
                bytes.extend_from_slice(&ty_len.to_be_bytes());
                bytes.extend_from_slice(node.ty.as_bytes());

                bytes.push(if node.deleted { 1 } else { 0 });

                let mut field_keys: Vec<&String> = node.fields.keys().collect();
                field_keys.sort();
                let field_len = field_keys.len() as u64;
                bytes.extend_from_slice(&field_len.to_be_bytes());
                for field_key in field_keys {
                    let key_len = field_key.len() as u64;
                    bytes.extend_from_slice(&key_len.to_be_bytes());
                    bytes.extend_from_slice(field_key.as_bytes());
                    if let Some(field_value) = node.fields.get(field_key) {
                        Self::write_value_canonical(&mut bytes, field_value);
                    }
                }
            }
        }

        let digest = Sha256::digest(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    pub fn create(&mut self, ty: &str) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let node = Node {
            id,
            ty: ty.to_string(),
            fields: HashMap::new(),
            deleted: false,
        };
        self.head_state.insert(id, node);

        let m = Mutation::CreateNode {
            id,
            ty: ty.to_string(),
        };
        self.pending_mutations.push(m);
        id
    }

    pub fn set(&mut self, id: NodeId, key: &str, value: Value) -> Result<(), MyosotisError> {
        let node = self
            .head_state
            .get(&id)
            .ok_or(MyosotisError::NodeNotFound(id))?;
        if node.deleted {
            return Err(MyosotisError::NodeDeleted(id));
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

    pub fn delete_field(&mut self, id: NodeId, key: &str) -> Result<(), MyosotisError> {
        let node = self
            .head_state
            .get(&id)
            .ok_or(MyosotisError::DeleteNonexistentNode(id))?;
        if node.deleted {
            return Err(MyosotisError::DeleteOnDeletedNode(id));
        }
        if !node.fields.contains_key(key) {
            return Err(MyosotisError::FieldNotFound(key.to_string()));
        }

        let m = Mutation::DeleteField {
            id,
            key: key.to_string(),
        };
        self.apply_mutation(&m)?;
        self.pending_mutations.push(m);
        Ok(())
    }

    pub fn delete_node(&mut self, id: NodeId) -> Result<(), MyosotisError> {
        let node = self
            .head_state
            .get(&id)
            .ok_or(MyosotisError::DeleteNonexistentNode(id))?;
        if node.deleted {
            return Err(MyosotisError::DeleteOnDeletedNode(id));
        }

        let m = Mutation::DeleteNode { id };
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

        let commit_id = self.commits.last().map(|c| c.id + 1).unwrap_or(1);
        let parent = self.commits.last().map(|c| c.id);

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

        let mutations = self.pending_mutations.clone();

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
                    base_state.insert(
                        *id,
                        Node {
                            id: *id,
                            ty: String::new(),
                            fields: HashMap::new(),
                            deleted: false,
                        },
                    );
                }
                Mutation::SetField { id, key, value } => {
                    let existing = base_state.get(id).ok_or(MyosotisError::Invariant(format!(
                        "set on missing node {}",
                        id
                    )))?;
                    if existing.deleted {
                        return Err(MyosotisError::NodeDeleted(*id));
                    }

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
                    if let Some(node) = base_state.get_mut(id) {
                        node.fields.insert(key.clone(), value.clone());
                    }
                }
                Mutation::DeleteField { id, key } => {
                    let existing = base_state
                        .get_mut(id)
                        .ok_or(MyosotisError::DeleteNonexistentNode(*id))?;
                    if existing.deleted {
                        return Err(MyosotisError::DeleteOnDeletedNode(*id));
                    }
                    if existing.fields.remove(key).is_none() {
                        return Err(MyosotisError::FieldNotFound(key.clone()));
                    }
                }
                Mutation::DeleteNode { id } => {
                    let existing = base_state
                        .get_mut(id)
                        .ok_or(MyosotisError::DeleteNonexistentNode(*id))?;
                    if existing.deleted {
                        return Err(MyosotisError::DeleteOnDeletedNode(*id));
                    }
                    existing.deleted = true;
                }
            }
        }

        let parent_hash = if let Some(last) = self.commits.last() {
            Some(last.hash)
        } else {
            self.genesis_state_hash
        };
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

        if self.commits.len().is_multiple_of(CHECKPOINT_INTERVAL) {
            let last = self
                .commits
                .last()
                .ok_or(MyosotisError::CorruptCommitChain(
                    "missing last commit after push".to_string(),
                ))?;
            let state_hash = Self::compute_state_hash(&self.head_state);
            self.checkpoints.push(Checkpoint {
                commit_id: last.id,
                commit_hash: last.hash,
                state_hash,
                state: self.head_state.clone(),
            });
        }

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
                    deleted: false,
                };
                self.head_state.insert(*id, node);
                Ok(())
            }
            Mutation::SetField { id, key, value } => {
                let node = self
                    .head_state
                    .get_mut(id)
                    .ok_or(MyosotisError::NodeNotFound(*id))?;
                if node.deleted {
                    return Err(MyosotisError::NodeDeleted(*id));
                }
                node.fields.insert(key.clone(), value.clone());
                Ok(())
            }
            Mutation::DeleteField { id, key } => {
                let node = self
                    .head_state
                    .get_mut(id)
                    .ok_or(MyosotisError::DeleteNonexistentNode(*id))?;
                if node.deleted {
                    return Err(MyosotisError::DeleteOnDeletedNode(*id));
                }
                if node.fields.remove(key).is_none() {
                    return Err(MyosotisError::FieldNotFound(key.clone()));
                }
                Ok(())
            }
            Mutation::DeleteNode { id } => {
                let node = self
                    .head_state
                    .get_mut(id)
                    .ok_or(MyosotisError::DeleteNonexistentNode(*id))?;
                if node.deleted {
                    return Err(MyosotisError::DeleteOnDeletedNode(*id));
                }
                node.deleted = true;
                Ok(())
            }
        }
    }

    pub fn replay(commits: &[Commit]) -> Result<HashMap<NodeId, Node>, MyosotisError> {
        Self::replay_from(HashMap::new(), commits)
    }

    pub fn replay_from(
        mut state: HashMap<NodeId, Node>,
        commits: &[Commit],
    ) -> Result<HashMap<NodeId, Node>, MyosotisError> {
        for commit in commits {
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
                            deleted: false,
                        };
                        state.insert(*id, node);
                    }
                    Mutation::SetField { id, key, value } => {
                        let existing = state.get(id).ok_or(MyosotisError::Invariant(format!(
                            "set before create {}",
                            id
                        )))?;
                        if existing.deleted {
                            return Err(MyosotisError::NodeDeleted(*id));
                        }

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
                    Mutation::DeleteField { id, key } => {
                        let node = state
                            .get_mut(id)
                            .ok_or(MyosotisError::DeleteNonexistentNode(*id))?;
                        if node.deleted {
                            return Err(MyosotisError::DeleteOnDeletedNode(*id));
                        }
                        if node.fields.remove(key).is_none() {
                            return Err(MyosotisError::FieldNotFound(key.clone()));
                        }
                    }
                    Mutation::DeleteNode { id } => {
                        let node = state
                            .get_mut(id)
                            .ok_or(MyosotisError::DeleteNonexistentNode(*id))?;
                        if node.deleted {
                            return Err(MyosotisError::DeleteOnDeletedNode(*id));
                        }
                        node.deleted = true;
                    }
                }
            }
        }

        Ok(state)
    }

    pub fn state_at_commit(
        &self,
        target_commit_id: u64,
    ) -> Result<HashMap<NodeId, Node>, MyosotisError> {
        let target_index = self
            .commits
            .iter()
            .position(|c| c.id == target_commit_id)
            .ok_or(MyosotisError::CommitNotFound(target_commit_id))?;

        let mut base_state: HashMap<NodeId, Node> = self.genesis_state.clone().unwrap_or_default();
        let mut start_index = 0usize;

        if let Some(cp) = self
            .checkpoints
            .iter()
            .filter(|c| c.commit_id <= target_commit_id)
            .max_by_key(|c| c.commit_id)
        {
            base_state = cp.state.clone();
            if let Some(pos) = self.commits.iter().position(|c| c.id == cp.commit_id) {
                start_index = pos + 1;
            } else {
                return Err(MyosotisError::InvalidCheckpoint);
            }
        }

        if start_index > target_index + 1 {
            return Err(MyosotisError::InvalidCheckpoint);
        }

        Self::replay_from(base_state, &self.commits[start_index..=target_index])
    }

    pub fn validate_with_mode(&self, verify_hashes: bool) -> Result<(), MyosotisError> {
        if let Some(genesis_state) = &self.genesis_state {
            let expected_hash = Self::compute_state_hash(genesis_state);
            if self.genesis_state_hash != Some(expected_hash) {
                return Err(MyosotisError::CorruptGenesisHash);
            }
        } else if self.genesis_state_hash.is_some() {
            return Err(MyosotisError::CorruptGenesisHash);
        }

        for (i, commit) in self.commits.iter().enumerate() {
            if i > 0 {
                let prev_id = self.commits[i - 1].id;
                if commit.id != prev_id + 1 {
                    return Err(MyosotisError::Invariant(format!(
                        "commit id {} is not sequential after {}",
                        commit.id, prev_id
                    )));
                }
            }

            if i == 0 {
                if commit.parent.is_some() {
                    return Err(MyosotisError::Invariant(
                        "first commit must have no parent".to_string(),
                    ));
                }
                if commit.parent_hash != self.genesis_state_hash {
                    return Err(MyosotisError::CorruptParentHash);
                }
            } else {
                let prev_id = self.commits[i - 1].id;
                if commit.parent != Some(prev_id) {
                    return Err(MyosotisError::Invariant(format!(
                        "commit {} has invalid parent {:?}, expected {}",
                        commit.id, commit.parent, prev_id
                    )));
                }

                let prev_hash = self.commits.get(i - 1).map(|c| c.hash).ok_or(
                    MyosotisError::CorruptCommitChain(
                        "missing previous commit for parent hash".to_string(),
                    ),
                )?;
                if commit.parent_hash != Some(prev_hash) {
                    return Err(MyosotisError::CorruptParentHash);
                }
            }

            if verify_hashes {
                let recomputed = Self::compute_commit_hash(
                    commit.parent_hash,
                    &commit.message,
                    &commit.mutations,
                );
                if commit.hash != recomputed {
                    return Err(MyosotisError::CorruptCommitHash);
                }
            }
        }

        for checkpoint in &self.checkpoints {
            let commit = self
                .commits
                .iter()
                .find(|c| c.id == checkpoint.commit_id)
                .ok_or(MyosotisError::CheckpointCommitMismatch)?;
            if commit.hash != checkpoint.commit_hash {
                return Err(MyosotisError::CheckpointCommitMismatch);
            }
            if verify_hashes {
                let recomputed_state_hash = Self::compute_state_hash(&checkpoint.state);
                if recomputed_state_hash != checkpoint.state_hash {
                    return Err(MyosotisError::CorruptCheckpointHash);
                }
            }
        }

        let state = if let Some(cp) = self.checkpoints.iter().max_by_key(|c| c.commit_id) {
            let start_index = self
                .commits
                .iter()
                .position(|c| c.id == cp.commit_id)
                .ok_or(MyosotisError::InvalidCheckpoint)?
                + 1;
            Self::replay_from(cp.state.clone(), &self.commits[start_index..])?
        } else {
            Self::replay_from(
                self.genesis_state.clone().unwrap_or_default(),
                &self.commits,
            )?
        };

        let max_id = state.keys().copied().max().unwrap_or(0);
        if self.next_node_id <= max_id {
            return Err(MyosotisError::Invariant(format!(
                "next_node_id {} <= max created id {}",
                self.next_node_id, max_id
            )));
        }

        if !self.head_state.is_empty() && self.head_state != state {
            return Err(MyosotisError::Invariant(
                "head_state does not match replayed state".to_string(),
            ));
        }

        Ok(())
    }

    pub fn validate(&self) -> Result<(), MyosotisError> {
        self.validate_with_mode(true)
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
