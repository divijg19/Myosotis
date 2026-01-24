# `Myosotis`

> **Deterministic, inspectable memory for long-lived programs**

`Myosotis` is a **memory engine**, not a database and not a runtime.
It provides programs with a persistent, versioned object graph that can be committed, rewound, inspected, and deterministically replayed over time.

While the initial user-facing API targets **Python**, `Myosotis` itself is **language-agnostic by design**.

---

## 1. Motivation

Modern programs increasingly behave like *ongoing processes* rather than short-lived scripts:

* AI agents that evolve goals
* Notebooks that act as living systems
* Simulations and games with rich internal state
* Local-first applications with long memory

Python excels at expressing *intent*, but it lacks a first-class concept of **long-lived, inspectable memory**.

Current options are insufficient:

* `pickle` → unsafe, opaque, brittle
* JSON → lossy, no identity
* Databases → heavy, external, semantically mismatched

**`Myosotis` fills this gap** by introducing a dedicated memory substrate.

---

## 2. What `Myosotis` Is (and Is Not)

### `Myosotis` **is**:

* A persistent object graph
* Versioned and append-only
* Deterministic and replayable
* Inspectable at every step
* Embedded (not a service)

### `Myosotis` **is not**:

* A Python runtime
* A garbage collector replacement
* A distributed database
* A cache
* A serialization format

Think of `Myosotis` as:

> **Git for your program's memory**

---

## 3. Core Concepts

### 3.1 Memory Graph

All state is represented as a graph:

* **Nodes** → entities (Agent, World, Document, etc.)
* **Edges** → attributes and relationships
* **Values** → primitives or references

Object identity is stable and explicit.

---

### 3.2 Commits

A commit records:

* A set of changes (diffs)
* Metadata (timestamp, label, optional user data)
* A pointer to the previous commit

Commits are:

* Append-only
* Immutable
* Ordered

---

### 3.3 Rewind & Replay

Memory can be:

* Rewound to any previous commit
* Replayed deterministically from the log
* Inspected at any historical point

This enables:

* Time-travel debugging
* Deterministic agent replay
* State introspection

---

### 3.4 Handles (Language Boundary)

User languages (e.g. Python) never own memory directly.

They interact via **handles**:

* Opaque references to memory nodes
* Stable across commits
* Cheap to copy

This keeps the engine language-agnostic.

---

## 4. Architecture Overview

```
┌──────────────────┐
│   User Language  │  (Python today)
└────────┬─────────┘
         │ handles / API
┌────────▼─────────┐
│   Engine Layer   │  (Go v1 → Rust v2)
│  - Graph logic   │
│  - Versioning    │
│  - Replay        │
└────────┬─────────┘
         │ raw memory interface
┌────────▼─────────┐
│ Substrate Layer  │  (Zig v3, optional)
│ - Allocators     │
│ - mmap storage   │
│ - snapshots      │
└──────────────────┘
```

---

## 5. Language Roadmap

### v1 — Go (Proof of Semantics)

* Purpose: validate the model
* Focus: correctness, clarity, API shape
* Tradeoff: GC-based, not fully deterministic internally

### v2 — Rust (Engine of Record)

* Purpose: safety, determinism, performance
* Focus: memory safety, replay guarantees, concurrency control

### v3 — Zig (Optional Substrate)

* Purpose: allocator control, layout determinism
* Focus: mmap, arenas, snapshot efficiency

---

## 6. Public Python API (Initial)

```python
from `Myosotis` import Memory

mem = Memory.open("state.mem")

agent = mem.create("Agent", name="Iris")
agent.goal = "Explore ideas"

mem.commit(label="initial goal")
```

### Rewind

```python
mem.rewind(steps=1)
```

### Inspect

```python
mem.inspect()
mem.diff(commit_a, commit_b)
```

---

## 7. Persistence Model

* Append-only event log
* Stable IDs
* Versioned on-disk format
* Replayable from scratch

No in-place mutation of stored history.

---

## 8. Determinism Guarantees

`Myosotis` guarantees:

* Ordered mutation application
* Stable replay results
* Explicit mutation boundaries

It does **not** guarantee:

* Real-time determinism
* Cross-machine bit-identical floating point (yet)

---

## 9. Concurrency Model (Initial Scope)

* Single-writer
* Multi-reader
* Explicit commit boundaries

True parallel mutation is deferred to v2+.

---

## 10. Introspection & Debugging

Built-in inspection tools:

* Memory statistics
* Commit graph visualization (text)
* Structured diffs
* Read-only historical views

These are first-class, not afterthoughts.

---

## 11. Is `Myosotis` Python-Specific?

**No.**

Python is the *first* client language, not the foundation.

`Myosotis` is:

* Language-agnostic at the engine level
* Designed around opaque handles
* Accessible via FFI or IPC

Potential future frontends:

* Rust
* Go
* Mojo
* JavaScript (via WASM)

Python is used because:

* It benefits the most today
* It excels as a control and intent language

---

## 12. Non-Goals

To keep scope sane, `Myosotis` explicitly does **not** aim to:

* Replace databases
* Be distributed initially
* Replace Python GC
* Automatically optimize user code
* Compete with existing runtimes

---

## 13. Project Philosophy

* Semantics before speed
* Inspectability over magic
* Explicit over implicit
* Memory as a first-class concept

---

## 14. Status

* v0.x: Go-based semantic exploration
* v1: Stable API, usable engine
* v2: Rust rewrite
* v3: Zig substrate (optional)

---

## 15. Summary

`Myosotis` gives programs something they currently lack:

> **A reliable way to remember, rewind, and reason about their own state over time.**

Python is the entry point.
The idea is universal.
