# `Myosotis`

> **Deterministic, inspectable memory for long-lived programs**

`Myosotis` is an **embedded, versioned object graph engine** designed for programs that need durable, replayable, and structurally transparent memory.

Unlike traditional databases or event logs, Myosotis is designed to model in-memory object identity and relationships directly, not tables or external schemas.

It is not a database, runtime or subservient cache.
Instead, it runs in-process as a **state substrate**.

Hence, the guiding goals and design principles are:
**Stable identity + graph-native + embedded + deterministic replay**

---

# 1. Why `Myosotis` Exists

Modern programs increasingly behave like ongoing processes rather than disposable scripts:

* AI agents with evolving goals
* Simulations and games with rich internal state
* Long-running services
* Interactive notebooks that accumulate knowledge
* Local-first applications with persistent context

Python (and many other languages) excel at expressing *intent*, but they lack a first-class concept of:

> Long-lived, inspectable, deterministic memory.

Current approaches are inadequate:

* `pickle` → unsafe, opaque, brittle
* JSON → lossy, no identity, no history
* Databases → external, schema-heavy, semantically mismatched
* Ad-hoc state management → fragile and hard to debug

`Myosotis` introduces a dedicated memory layer that treats **state as a structured, versioned, replayable graph**.

---

# 2. `Myosotis` is:

* An **embedded object graph**
* Append-only and versioned
* Deterministic by construction
* Replayable from a mutation log
* Fully inspectable at every commit
* Language-agnostic at the engine layer

Think of it as:

> **Git for structured program state**

But instead of tracking text files, it tracks a program’s internal memory graph.

---

# 3. What `Myosotis` Is Not

To keep scope precise and architectural integrity intact, `Myosotis` does **not** aim to:

* Replace databases
* Replace Python’s garbage collector
* Act as a distributed system
* Provide automatic scaling
* Optimize user code execution
* Replace programming language runtimes
* Act as a generic serialization format

It solves one problem:

> Making program memory persistent, versioned, and inspectable.

---

# 4. Core Design Principles

## 4.1 Determinism First

Given:

* An initial state
* An ordered sequence of mutations

The resulting state will be identical across replays.

Mutation order is explicit.
History is append-only.
State is never implicitly rewritten.

---

## 4.2 Stable Identity

Each entity in memory:

* Has a stable, persistent ID
* Maintains referential integrity
* Is independent of language-level object addresses

Identity does not disappear between runs.

---

## 4.3 Versioned State

State is never mutated in-place historically.

Instead:

* Changes are recorded as commits
* Commits are immutable
* History is replayable
* Any past commit can be reconstructed

---

## 4.4 Inspectability Over Magic

Memory can be:

* Enumerated
* Traversed
* Diffed between commits
* Viewed historically

No opaque blobs.
No hidden mutation.

---

## 4.5 Embedded by Default

`Myosotis` runs:

* In-process
* As a single engine
* Backed by a file

It is not a background service.

---

# 5. Architecture

`Myosotis` is implemented in **Rust** from the ground up to ensure:

* Memory safety
* Deterministic mutation ordering
* Stable persistence guarantees
* Strong internal invariants

Architecture overview:

```
┌──────────────────┐
│   Client Layer   │  (Python, Rust, others)
└────────┬─────────┘
         │ Handles / API
┌────────▼─────────┐
│   Engine Core    │  (Rust)
│  - Object graph  │
│  - Versioning    │
│  - Commit log    │
│  - Replay logic  │
│  - Diff engine   │
└────────┬─────────┘
         │
┌────────▼─────────┐
│  Storage Layer   │
│  - Append-only   │
│  - Log structured│
│  - Crash safe    │
└──────────────────┘
```

The Rust engine is the **single source of truth**.

---

# 6. Data Model

## 6.1 Nodes

Each node contains:

* `NodeId` (u64)
* `Type` (string)
* `Fields` (map of string → value)
* `deleted` tombstone flag (soft-delete marker)

Deletion model:

* Nodes are never physically removed from state
* NodeId is never reused, even after deletion
* No cascading delete is performed

## 6.2 Values

Supported value types:

* Integer
* Float
* Boolean
* String
* Reference (NodeId)
* List
* Map

Arbitrary host-language objects are not stored directly.

---

# 7. Commits

A commit records:

* A set of mutations (diff)
* Metadata (timestamp, optional label)
* Pointer to the previous commit

Commits are:

* Immutable
* Append-only
* Totally ordered (linear history in initial versions)

---

# 8. Replay Model

The engine can:

* Reconstruct any historical state
* Replay from genesis
* Produce deterministic results

This enables:

* Time-travel debugging
* Agent replay
* Simulation auditing
* State introspection

---

# 9. Concurrency Model (Initial Scope)

* Single writer
* Multiple readers
* Explicit commit boundaries

Parallel mutation is out of scope for initial releases.

---

# 10. CLI (Initial Surface)

The first interface to `Myosotis` is a Rust-based CLI.

Example:

```bash
myo init state.myo
myo create Agent
myo set <node_id> goal "Explore"
myo commit "initial goal"
myo history
myo show <node_id> --at <commit>
myo diff <commit_a> <commit_b>
```

The CLI uses the same engine as the library.

---

# 11. Python Integration (After Engine Stabilizes)

Python is the first client language.

Example:

```python
from myosotis import Memory

mem = Memory.open("state.myo")

agent = mem.create("Agent", name="Iris")
agent.goal = "Explore ideas"

mem.commit(label="initial goal")
```

Python interacts via **handles**, not raw memory ownership.

The engine remains language-agnostic.

---

# 12. Persistence Model

* Append-only commit log
* Stable on-disk format
* Crash-safe writes
* Deterministic replay from file
* Deterministic full-state checkpoints (derived cache layer)
* Bounded replay cost by starting from nearest checkpoint
* Checkpoints are integrity-verified on load
* Commit log remains canonical source of truth
* Tombstones are persisted and included in deterministic state hashing
* Deterministic prefix compaction via genesis snapshots

Compaction notes:

* Compaction rewrites historical prefix into a canonical genesis snapshot
* Observable latest state is preserved exactly
* Tombstoned nodes are preserved (no tombstone pruning)
* Commit log after compaction remains hash-chain verified

---

# File Format Specification (v1)

Top-level file header fields:

* `magic`: must be `"MYOSOTIS"`
* `format_version`: must be `1`

Top-level schema fields:

* `magic`
* `format_version`
* `genesis_state` (optional)
* `genesis_state_hash` (optional)
* `commits`
* `checkpoints`
* `next_node_id`

Compatibility policy:

* `format_version` increments only for breaking storage changes.
* Files with `format_version` greater than supported are refused.
* v0.5.0 legacy files (without header) are loaded and migrated on next write.
* Header migration does not alter commit/state semantics or hash algorithms.

Forward-compat guardrail:

* Unknown future format versions are explicitly rejected.

Serialization discipline:

* All writes include `magic` and `format_version`.
* Field ordering is emitted deterministically from the serializer-backed struct layout.

History is never mutated.

---

# 13. Guarantees

`Myosotis` guarantees:

* Deterministic mutation ordering
* Stable identity across sessions
* Immutable historical commits
* Replayable state reconstruction

It does not guarantee:

* Distributed consistency
* Cross-machine floating-point bit identity
* Real-time determinism

---

# 14. Roadmap

### v0

* Rust engine
* CLI interface
* Linear history
* Basic diff support

### v1

* Stable on-disk format
* Full diff engine
* Python bindings
* Inspection tooling

Future optimization layers (allocator tuning, snapshot optimization, substrate experimentation) remain possible but are not required for correctness.

---

# 15. Philosophy

* Determinism before convenience
* Semantics before speed
* Inspectability before abstraction
* Stability before expansion

`Myosotis` treats memory as a first-class system component, not an implementation detail.

---

# 16. Summary

`Myosotis` provides something most programs currently lack:

> A reliable way to remember, rewind, and reason about their own internal state over time.

It is:

* Embedded
* Deterministic
* Versioned
* Inspectable

Python is the first client.

The engine is universal.
