# Consul Index System

This document describes ALL index types used in Consul, their purposes, relationships, and how they should be implemented in Batata's Consul compatibility layer.

## Overview

Consul uses a unified index system rooted in the **Raft log index**. Every write operation that goes through Raft consensus produces a monotonically increasing log index. This single source of truth propagates into multiple index fields across different data structures.

```
Raft Log Index N (from Raft leader)
    |
    +-> DirEntry.CreateIndex = N      (on creation only)
    +-> DirEntry.ModifyIndex = N      (on every write)
    +-> DirEntry.LockIndex++          (on lock acquire, separate counter)
    +-> IndexEntry{"kvs", N}          (per-table high water mark)
    +-> IndexEntry{"tombstones", N}   (on deletes)
    +-> IndexEntry{"services", N}     (catalog writes)
    +-> IndexEntry{"node.NAME", N}    (per-node index)
    +-> X-Consul-Index: N             (HTTP response header)
```

## 1. X-Consul-Index (HTTP Response Header)

| Property | Value |
|----------|-------|
| Type | HTTP response header |
| Data type | uint64 |
| Minimum value | 1 (never 0) |
| Source | Per-table high water mark from Raft log index |

### How it's set

```
Client request
  -> Server executes query
  -> QueryMeta.SetIndex(tableMaxIndex)
  -> setMeta(resp, queryMeta)
  -> setIndex(resp, index)
  -> HTTP header: X-Consul-Index: N
```

Key source files:
- `agent/http.go` — `setIndex()`, `setMeta()`
- `agent/blockingquery/blockingquery.go` — blocking query loop

### Purpose

- Provides the client with a "cursor" for subsequent blocking queries
- Client sends `?index=N` on next request to long-poll for changes
- Server blocks until its index > N, then returns new data

### Value semantics

The value is the **maximum Raft index** across all tables relevant to the query:

| Query Type | Tables Used |
|------------|-------------|
| KV read | `max("kvs", "tombstones")` |
| Service list | `max("services")` |
| Node catalog | `max("nodes")` |
| Session | `max("sessions")` |

## 2. KVPair Index Fields

Every KV entry (`DirEntry` in Consul, `KVPair` in Batata) has these index fields:

### CreateIndex

| Property | Value |
|----------|-------|
| Set when | Key first created |
| Updated | Never (immutable after creation) |
| Source | Raft log index at creation time |

```go
// From kvs.go kvsSetTxn:
if existing != nil {
    entry.CreateIndex = existing.CreateIndex  // preserve
} else {
    entry.CreateIndex = idx  // idx = Raft log index
}
```

### ModifyIndex

| Property | Value |
|----------|-------|
| Set when | Every write to the key (PUT, acquire, release) |
| Updated | On each modification |
| Source | Raft log index of the modification |
| Used for | CAS operations, blocking queries |

```go
// Always set on write:
entry.ModifyIndex = idx  // idx = Raft log index
```

CAS (Check-And-Set) compares the client-provided `?cas=N` against `entry.ModifyIndex`:
```go
if cidx != 0 && existing.ModifyIndex != cidx {
    return false  // CAS failed
}
```

### LockIndex

| Property | Value |
|----------|-------|
| Set when | Session lock acquired on the key |
| Updated | Incremented by 1 on each new lock acquisition |
| Source | Internal counter (NOT Raft index) |
| Reset | Never decremented |

```go
// On lock acquire (unlock->lock transition):
entry.LockIndex++
// On re-acquire by same session: no change
// On release: LockIndex stays the same
```

## 3. Blocking Query Mechanism

### Client flow

```
1. GET /v1/kv/mykey
   <- X-Consul-Index: 42
   <- body: {"Key":"mykey","Value":"...","ModifyIndex":42}

2. GET /v1/kv/mykey?index=42&wait=5m
   <- blocks until index > 42 or timeout
   <- X-Consul-Index: 57
   <- body: {"Key":"mykey","Value":"new","ModifyIndex":57}
```

### Server-side logic (blockingquery.go)

```go
func Query(backend BlockingQueryBackend, opts RequestOptions, meta ResponseMeta, fn QueryFn) error {
    minQueryIndex := opts.GetMinQueryIndex()  // from ?index=N
    maxQueryTime  := opts.GetMaxQueryTime()   // from ?wait=Xs

    for {
        fn(ws)  // execute query, populate response and watch set

        if meta.GetIndex() > minQueryIndex {
            return nil  // DATA CHANGED — unblock!
        }

        // Block until watch fires or timeout
        ws.WatchCtx(ctx)
    }
}
```

### Key rules

1. **Compare `>` not `>=`**: Unblock when `responseIndex > requestIndex`
2. **Minimum index = 1**: Never return 0 (prevents busy loops)
3. **Default wait = 5 minutes**: If `?wait` not specified
4. **Max wait = 10 minutes**: Capped at 600 seconds
5. **Watch-based**: Uses memdb WatchSet for efficient change detection (not polling)

## 4. Per-Table Index (High Water Marks)

Consul maintains an **index table** with entries like:

```go
type IndexEntry struct {
    Key   string  // table name, e.g. "kvs", "services", "tombstones"
    Value uint64  // highest Raft index that affected this table
}
```

### How table indexes work

Every write operation updates the table index:

```go
// After inserting a KV entry:
tx.Insert(tableIndex, IndexEntry{"kvs", raftLogIndex})

// After deleting a KV entry (tombstone):
tx.Insert(tableIndex, IndexEntry{"tombstones", raftLogIndex})
```

Reads query the max across relevant tables:

```go
func kvsMaxIndex(tx) uint64 {
    return maxIndexTxn(tx, "kvs", "tombstones")
}
```

### Why tombstones matter

Without tombstones, deleting the last key in a table would make the table index go backwards (to the ModifyIndex of the previous remaining key). Tombstones prevent this by recording the deletion's Raft index.

### Catalog granular indexes

The catalog maintains fine-grained indexes for efficient watches:

| Index Key | Scope |
|-----------|-------|
| `services` | All services |
| `service.NAME` | Specific service (all instances) |
| `node.NAME` | Specific node |
| `service_kind.KIND` | Services of a specific kind |
| `checks` | All health checks |

## 5. Session Indexes

Sessions have the same `RaftIndex` embedded struct:

| Field | Description |
|-------|-------------|
| `CreateIndex` | Raft index when session was created |
| `ModifyIndex` | Raft index when session was last modified (renew, etc.) |

Session creation/destruction also updates the `sessions` table index, which affects blocking queries on session endpoints.

## 6. Complete Write Flow (KV PUT example)

```
Client: PUT /v1/kv/mykey?cas=42
    |
    v
HTTP Handler (kvs_endpoint.go)
    |
    v
Raft Leader: raftApply(KVSRequestType, args)
    |-- Replicates to followers
    |-- Log entry at index N
    |
    v
FSM.Apply(log)  // log.Index = N
    |
    v
state.KVSSet(idx=N, entry)
    |
    v
kvsSetTxn(tx, idx=N, entry):
    |-- CAS check: existing.ModifyIndex == 42? (if cas specified)
    |-- entry.CreateIndex = existing.CreateIndex (preserve)
    |-- entry.ModifyIndex = N
    |-- tx.Insert("kvs", entry)
    |-- tx.Insert(tableIndex, {"kvs", N})
    |
    v
Return to HTTP handler
    |-- QueryMeta.Index = kvsMaxIndex(tx) = N
    |-- setMeta(resp, queryMeta)
    |-- X-Consul-Index: N
    |
    v
Client receives:
    Header: X-Consul-Index: N
    Body: {"CreateIndex":..., "ModifyIndex":N, "LockIndex":...}
```

## 7. Implications for Batata

### Current issues

Batata has **two separate index systems** that are not aligned:
1. `ConsulKVService.index` — local AtomicU64 counter for KV operations
2. `ConsulIndexProvider` — intended for X-Consul-Index header

This causes:
- `X-Consul-Index` header returns a value from one counter
- `KVPair.ModifyIndex` uses a different counter
- Blocking queries compare against the wrong index

### Correct design

All indexes should originate from a **single monotonic counter**:

```
Single Global Counter (or Raft log index in cluster mode)
    |
    +-> KVPair.CreateIndex (on creation)
    +-> KVPair.ModifyIndex (on every write)
    +-> X-Consul-Index header (on every response)
    +-> Blocking query comparison target
```

### Implementation options

**Option A: Raft-based (matches Consul exactly)**
- Use `raft.last_applied_index()` as the global index
- All CreateIndex/ModifyIndex values come from the Raft log
- Blocking queries wait for Raft index to advance
- Challenge: Raft metrics may lag slightly after `client_write()` returns

**Option B: Local counter with Raft sync (pragmatic)**
- Single `AtomicU64` counter shared between KV service and index provider
- Incremented on every successful write (after Raft commit)
- Used for both ModifyIndex and X-Consul-Index
- Blocking queries wait on this counter via `tokio::sync::Notify`
- Challenge: Counter values differ across cluster nodes (but this is OK since clients typically talk to one node)

**Option C: Hybrid (recommended)**
- Use a single `AtomicU64` for all index operations
- Initialize from Raft `last_applied_index` on startup
- Increment locally after each successful write
- All KV fields and headers use this same counter
- `Notify` triggers blocking query unblock

### Key design rules

1. **One counter to rule them all**: `CreateIndex`, `ModifyIndex`, and `X-Consul-Index` must use the same counter
2. **Atomic increment + notify**: `counter.fetch_add(1)` then `notify.notify_waiters()`
3. **Never return index 0**: Minimum is 1
4. **Blocking condition**: `current_index > client_index` (strict greater than)
5. **Tombstones**: Track deletion indexes to prevent index regression
6. **CAS uses ModifyIndex**: `?cas=N` compares against `entry.ModifyIndex`
7. **LockIndex is separate**: Not a Raft index, just an acquisition counter
