# zelper Basic Design

Status: Architecture and interface direction; detailed design is delegated to the coding agent after research/prototyping.

## 1. Design thesis

`zelper` is a **semantic orchestration layer** over Zellij.

The abstraction boundary is:

```text
user / shell / coding agent
          |
          v
       zelper
  intent + selection
  planning + validation
  aggregation + formatting
          |
          v
 Zellij public CLI/actions
          |
          v
 running Zellij sessions
```

`zelper` must not simply rename Zellij commands. It should translate intent into one or more Zellij operations.

Examples:

- "read all coding-agent panes" may require list/query + multiple screen dumps + aggregation.
- "resize these panes equally" may require geometry discovery + repeated directional resize operations.
- "remap 7 panes using a 3-slot layout" may require state capture + tab creation + pane movement + repeated layout application + validation.

## 2. Architectural layers

The codebase SHOULD be divided into the following conceptual layers even if module names differ.

### 2.1 CLI layer

Responsibilities:

- parse verb-first grammar
- validate syntactic conflicts
- produce help/completion
- map arguments/options into typed application requests
- no raw subprocess orchestration logic

### 2.2 Selection/resolution layer

Responsibilities:

- resolve positional targets
- resolve `--tab`, `--command`, `--cwd`, `--all`, etc.
- detect ambiguity
- return deterministic typed target sets

Core rule:

```text
User selector -> Vec<ResolvedPane> / Vec<ResolvedTab>
```

Single-target commands validate that exactly one target is returned. Set-oriented commands consume the full set.

### 2.3 Zellij adapter

A narrow boundary around the actual `zellij` executable.

Responsibilities:

- invoke Zellij
- detect version/features
- parse structured outputs
- normalize IDs and metadata
- expose typed primitives to the rest of the program
- isolate command-line syntax differences across supported Zellij versions

No application command should manually construct arbitrary `zellij action ...` command lines outside this adapter.

Candidate conceptual interface:

```text
ZellijBackend
  version()
  sessions()
  tabs(session)
  panes(session/tab)
  dump_screen(pane, mode)
  write_text(pane, text)
  write_key(pane, key)
  rename_pane(...)
  rename_tab(...)
  new_pane(...)
  new_tab(...)
  close_pane(...)
  close_tab(...)
  resize(...)
  move/reparent capability ...
  override_layout(...)
```

This list is illustrative; detailed design must align with real public Zellij capabilities.

### 2.4 Domain model

Application code should work with normalized domain types rather than raw JSON maps.

Likely types include:

```text
SessionRef / SessionState
TabId / TabRef / TabState
PaneId / PaneRef / PaneState
Geometry
PaneKind (terminal/plugin/...)
LayoutRef
LayoutDefinition
TargetSet<T>
OperationPlan
OperationResult
```

Do not assume raw Zellij IDs are stable integer types until researched. Use newtypes/string-safe representations if necessary.

### 2.5 Planning layer

Mutating higher-level operations SHOULD first produce an explicit plan.

```text
Request
  -> resolve current state
  -> validate capability and constraints
  -> OperationPlan
  -> optionally render dry-run
  -> execute
  -> verify/result
```

This is mandatory architecture for `remap` and strongly preferred for bulk resize/rename/remove.

### 2.6 Output layer

Human and machine output must be separated from domain logic.

Human:

- compact tables/sections
- meaningful pane/tab labels
- grouped multi-pane read output
- useful change plans

Machine:

- JSON
- stable keys and explicit IDs
- no ANSI decoration unless explicitly requested

## 3. CLI grammar direction

### 3.1 Root

```text
zelper <verb> ...
```

The top level should be small and verb-oriented.

A candidate starting vocabulary is:

```text
list
read
send
rename
resize
remap
add
remove
```

This is NOT frozen. The detailed-design phase must test the vocabulary against all required operations and reduce/adjust it if a more coherent grammar exists.

Avoid adding top-level commands for every low-level Zellij action.

### 3.2 Common-operand principle

Normal usage should use positional operands.

Examples illustrating the intended ergonomics:

```text
zelper read 12
zelper send 12 y
zelper rename 12 worker-1
zelper remap agents
```

When a command can target different object types, the grammar may include a small noun operand where ambiguity requires it:

```text
zelper rename pane 12 worker-1
zelper rename tab 3 agents
```

Detailed design should minimize such nouns but must prefer clarity over clever inference.

### 3.3 Alternative resolution via options

Alternate target/source forms use options:

```text
zelper read --tab agents
zelper read --command codex
zelper send --all y
zelper remap --path ./test.kdl
zelper remap --inline 'layout { ... }'
```

Do not make the positional grammar polymorphic by secretly interpreting arbitrary strings as IDs, names, paths, DSLs, and query expressions at the same position without explicit disambiguation.

### 3.4 Shared option semantics

Shared concepts must be centralized in code and documentation.

Candidate categories:

```text
Target selection:
  --id
  --name
  --tab
  --command
  --cwd
  --all

Output:
  --json
  --quiet

Mutation safety:
  --dry-run
  --yes / --force   (only if detailed design establishes need)
```

The final names are a detailed-design decision, but inconsistent synonyms across commands are forbidden without justification.

## 4. Command capability design

## 4.1 `list`

Purpose: discovery, not exhaustive diagnostics.

Candidate forms:

```text
zelper list sessions
zelper list tabs
zelper list panes
zelper list layouts
```

Output should make IDs/names and enough identifying metadata available for later commands.

Do not overload `list` with full screen content.

## 4.2 `read`

Purpose: retrieve terminal output for one/set of panes.

Conceptual forms:

```text
zelper read PANE
zelper read PANE PANE ...
zelper read --tab TAB
zelper read --command COMMAND
zelper read --all
```

Candidate behavior options:

```text
--full      include full scrollback
--ansi      preserve ANSI if supported/needed
--tail N    wrapper-level tail convenience
--json
```

Exact flags should be derived from a coherent model and current Zellij capabilities.

Human multi-pane output should clearly delimit panes.

JSON should return an array/object carrying pane identity and output independently.

## 4.3 `send`

Purpose: send input to one/set of panes.

Conceptual forms:

```text
zelper send PANE TEXT
zelper send PANE PANE ... -- TEXT
zelper send --tab agents TEXT
zelper send --command codex TEXT
```

The argument grammar must avoid ambiguity between multiple targets and text; detailed design should decide whether `--` is required/recommended or targets are repeated via options.

Semantics:

- text send does not silently imply Enter
- explicit `--enter` may append Enter
- key mode should be available without pretending arbitrary key chords are text
- broadcast operation reports result per pane

Do not hide partial failure.

## 4.4 `rename`

Purpose: rename pane/tab with a consistent external command.

Conceptual:

```text
zelper rename pane PANE NAME
zelper rename tab TAB NAME
```

Bulk/generated naming may be added after simple semantics are proven.

## 4.5 `resize`

Purpose: higher-level sizing, not merely a spelling of Zellij `resize`.

Detailed design must produce a capability table for:

- direct directional increment/decrement
- equalize selected tiled panes
- target width/height or approximate percentages
- scope by tab or selected set
- floating vs tiled panes
- stacked panes

Implementation may use iterative low-level resize operations but must have termination bounds and verification. It must not loop indefinitely attempting an impossible exact geometry.

Where exact percentage sizing cannot be guaranteed by public Zellij APIs, the CLI/documentation must state the achieved/approximate semantics.

## 4.6 `remap`

### 4.6.1 Public semantic contract

Primary form:

```text
zelper remap LAYOUT_NAME
```

Alternative layout sources:

```text
zelper remap --path PATH
zelper remap --inline SPEC
```

Layout sources are mutually exclusive.

### 4.6.2 Planning model

A remap plan should conceptually include:

```text
RemapPlan
  source panes (ordered)
  source tabs
  chosen layout
  slots per layout instance
  required number of instances/tabs
  target tabs (reuse/create)
  pane -> target tab/slot assignment
  structural operations
  preservation policy
  verification checks
```

### 4.6.3 Default overflow rule

If the layout can place `N` panes and the selected source set contains `M` panes:

```text
instances = ceil(M / N)
```

The layout is repeated across tabs until all panes are assigned.

Example: 6 panes + 3-slot layout -> 2 layout instances/tabs.

Example: 7 panes + 3-slot layout -> 3 instances; the final instance contains one assigned pane and remaining layout capacity is unused according to the selected/defined policy.

The exact handling of unused slots and how Zellij layout placeholders interact with existing panes must be experimentally verified.

### 4.6.4 Preservation

Existing running terminal panes are identities that should survive remap.

The plan must distinguish:

```text
pane identity/process preservation
layout slot geometry
pane ordering
```

Do not equate "apply layout" with "start commands declared by the layout".

Research Zellij's `override-layout --retain-existing-terminal-panes` and swap-layout behavior first. Use native primitives where they produce the required preservation semantics.

### 4.6.5 Cross-tab issue

A key research item is how existing running panes can be transferred/reassigned across tabs through supported Zellij actions. The agent must prototype this before finalizing the algorithm.

If public Zellij operations cannot preserve a pane while moving it across a tab boundary, the design must explicitly document the limitation and find the least destructive supported architecture rather than silently recreating the process.

### 4.6.6 Ordering

Remap needs a deterministic input order. Candidate bases to research:

- current tab order + visual pane traversal
- Zellij's returned pane ordering
- explicit user order
- persisted logical order

The final choice must be documented and testable.

### 4.6.7 Tab management

Detailed design must define:

- when existing tabs are reused
- when new tabs are created
- what happens to source tabs that become empty
- how generated tabs are named
- behavior with tabs outside the remap scope
- rollback/recovery on failure mid-plan

## 4.7 `add`

Conceptual forms:

```text
zelper add pane
zelper add tab
```

Likely options:

```text
--count N
--layout NAME
--tab TARGET
```

Exact grammar must preserve verb-first consistency.

## 4.8 `remove`

Conceptual forms:

```text
zelper remove pane PANE...
zelper remove tab TAB...
```

Potential useful policies:

```text
--empty-tabs
--dry-run
```

Bulk destructive actions require explicit safety semantics suitable for both human and automated use.

## 5. Layout abstraction

A layout is a logical resource, not synonymous with a filepath.

The application should resolve a `LayoutRef` through a resolver.

Conceptual sources:

```text
name -> normal/default resolution
path -> explicit --path
inline KDL -> explicit --inline
```

Named resolution may include Zellij's normal layout directory and optionally zelper-specific configuration. Detailed design must define precedence and collision handling.

Internally, `zelper` should parse only as much of the KDL layout as necessary for planning. Avoid building a second complete Zellij layout engine if native Zellij can perform placement.

However, remap planning may require knowledge of usable pane slot counts and constraints. Determine the minimum robust parsing/model needed.

## 6. State snapshots and IDs

Every broad mutation should begin with a state snapshot sufficient to:

- resolve targets
- display dry-run
- detect changed state before execution where useful
- verify outcome
- aid failure diagnostics

IDs are Zellij-owned identifiers. `zelper` should not invent a second primary ID system in v1.

Names are convenience identifiers and may be ambiguous. Ambiguity must result in a clear error unless the operation explicitly accepts sets.

## 7. Error model

Errors should distinguish at least:

```text
usage/parse error
Zellij unavailable
unsupported Zellij version/feature
no target matched
ambiguous target
layout not found/invalid
preflight planning failure
Zellij operation failure
partial bulk failure
postcondition verification failure
```

Use non-zero exit statuses appropriate for scripting. Detailed design should assign codes/classes without overengineering a huge taxonomy.

Human diagnostics should state what was attempted and what the user can inspect next.

JSON mode should have structured error fields if JSON output is requested.

## 8. Concurrency and ordering

Read-only calls may be parallelized when safe and useful.

Mutation calls must preserve deterministic ordering when operations can affect subsequent IDs/geometry/layout.

Do not parallelize structural changes merely for speed.

For broadcast `send`, detailed design may choose sequential or bounded parallel writes, but ordering and partial failure behavior must be deterministic/documented.

## 9. Suggested Rust implementation shape

Not mandatory naming, but a useful target structure:

```text
src/
  main.rs
  cli/
    mod.rs
    args.rs
  app/
    list.rs
    read.rs
    send.rs
    rename.rs
    resize.rs
    remap.rs
    add.rs
    remove.rs
  domain/
    session.rs
    tab.rs
    pane.rs
    layout.rs
    plan.rs
    selector.rs
  zellij/
    mod.rs
    process.rs
    parser.rs
    capabilities.rs
  layout/
    resolver.rs
    parser.rs
  output/
    human.rs
    json.rs
  error.rs
```

Potential Rust ecosystem candidates to evaluate rather than blindly adopt:

- `clap` + `clap_complete`
- `serde` + `serde_json`
- `miette` / `thiserror` / `anyhow` according to error-layer needs
- a mature KDL parser if parsing is needed
- `assert_cmd` / snapshot testing / temporary test harnesses for CLI tests

Detailed design should prefer typed explicit application code over an elaborate framework architecture.

## 10. Test architecture

### 10.1 Pure unit tests

Must cover:

- selector resolution
- ambiguity rules
- layout source precedence/conflicts
- remap assignment math/order
- operation plan generation
- JSON schemas/serialization
- CLI parse grammar

### 10.2 Fake-backend tests

The `ZellijBackend` boundary should permit a deterministic fake/mock implementation.

Use it to test:

- multi-step planning/execution
- partial failure
- remap sequencing
- resize convergence logic
- rollback/best-effort recovery logic

### 10.3 Real integration tests

Run against an actual Zellij binary in isolated sessions.

Must verify the dangerous assumptions, especially:

- list/state parsing
- pane screen dump
- targeted input
- rename
- higher-level resize result
- pane/tab creation/deletion
- runtime layout override
- retention of running pane/process during layout override
- cross-tab remap feasibility
- overflow onto repeated layout instances

Integration tests should use uniquely named temporary sessions and aggressively clean them up.

### 10.4 Compatibility tests

If supporting multiple Zellij versions, run a small capability suite against the minimum and current supported versions in CI where practical.

## 11. Documentation requirements

The implementation repository must include:

- concise README with purpose and examples
- generated/maintained CLI `--help`
- command reference
- remap semantics with diagrams/examples
- selector/target-resolution rules
- JSON output examples
- compatibility/minimum Zellij version
- known limitations, especially pane movement/preservation boundaries

## 12. Design review gates

Implementation of remap/resize must not start from assumptions.

Before coding those algorithms, the detailed design must record experimentally verified answers to:

1. What exact pane/tab metadata is available programmatically?
2. What exact operations accept explicit pane/tab IDs?
3. Can a running pane be moved across tabs without recreating the process?
4. How does `override-layout --retain-existing-terminal-panes` assign existing panes to slots?
5. What happens when existing panes exceed/fall short of layout slots?
6. How do floating/plugin/stacked panes behave?
7. Can resize reach a target geometry deterministically and how is terminal size rounded?

These findings become part of the detailed design and tests.
