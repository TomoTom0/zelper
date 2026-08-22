# zelper Development Plan for Coding Agent

This project MUST use an evidence-driven development sequence. Do not jump directly from these basic documents into implementation.

## Phase 1 — Research

### Goals

Establish the real capabilities and limitations of the current Zellij CLI/API before detailed design.

### Required work

1. Identify current Zellij version and upstream current version.
2. Review official documentation and, where needed, relevant Zellij source code/tests.
3. Inventory public commands/actions relevant to:
   - session/tab/pane listing
   - pane/tab IDs
   - structured output
   - screen/scrollback dump
   - targeted text/key input
   - pane/tab rename
   - pane/tab creation/deletion
   - pane resize
   - pane movement, especially cross-tab
   - layout discovery
   - `override-layout`
   - retaining existing panes
   - inline layout strings
   - swap layouts
4. Build small throwaway experiments for behavior that documentation does not prove.
5. Record exact outputs and limitations; do not rely on memory or assumptions.

### Mandatory research artifact

Create:

```text
docs/research/zellij-capabilities.md
```

Include a matrix:

```text
Capability | Public primitive | Explicit target support | Structured output | Verified version | Notes/limitations
```

Also include reproducible commands for every important experimental claim.

## Phase 2 — Requirements review

Review `requirements.md` against research findings.

For each requirement classify:

```text
supported directly
supported by orchestration
supported approximately/with limitation
not possible through supported Zellij interfaces
```

Do not silently drop requirements.

If a requirement is impossible, document the constraint and propose the closest design that preserves the original intent.

Produce:

```text
docs/design/requirements-traceability.md
```

Every P0 requirement must map to a detailed-design section and eventually to tests.

## Phase 3 — Detailed design

Produce detailed design BEFORE implementation.

Required artifact:

```text
docs/design/detailed-design.md
```

It must include at least:

### CLI grammar

- frozen top-level verb list for v1
- full syntax tree
- positional operand rules
- shared target options
- ambiguity rules
- mutually exclusive options
- examples and counterexamples
- shell completion model

### Domain/state model

- normalized session/tab/pane types
- identity semantics
- geometry model
- pane kinds
- layout reference and resolution model

### Backend interface

- typed Zellij adapter API
- subprocess invocation rules
- structured parsing
- version/capability detection

### Output contracts

- human formatting rules
- JSON structures/schema examples
- stable vs non-stable fields

### Operation designs

For each P0 verb:

- request model
- target resolution
- preconditions
- plan
- backend actions
- postconditions
- failure handling
- dry-run output where applicable

### Remap algorithm

Must be exceptionally detailed:

- source-set resolution
- pane ordering
- layout slot analysis
- layout instance count
- tab reuse/create rules
- pane transfer/reposition sequence
- use of override/swap layout primitives
- preservation guarantee and limitations
- overflow/repetition
- leftover/empty tab behavior
- floating/plugin pane policy
- failure midway
- verification
- dry-run plan

Include worked examples for at least:

- 1 pane into 3-slot layout
- 3 into 3
- 4 into 3
- 6 into 3
- 7 into 3
- multiple source tabs
- a failure during the second layout instance

### Resize algorithm

Define:

- exact vs approximate sizing
- equalization algorithm
- iteration bounds
- geometry refresh policy
- impossible-target behavior

### Safety model

- destructive confirmation/noninteractive behavior
- dry-run
- partial failure
- cleanup

## Phase 4 — Design review

Perform a formal self-review before implementation.

Create:

```text
docs/design/design-review.md
```

Review against:

- requirements completeness
- CLI structural consistency
- whether commands are semantic wrappers rather than aliases
- first-argument verb principle
- positional-primary / option-alternative principle
- multi-target consistency
- remap preservation
- failure/rollback realism
- testability
- compatibility assumptions

List all identified issues and their disposition.

A design-review issue cannot be considered resolved merely because implementation could work around it.

## Phase 5 — Test design before implementation

Create tests/specifications before production implementation for core behavior.

Required artifact:

```text
docs/testing/test-plan.md
```

At minimum specify:

- CLI parser tests
- target resolver tests
- backend parser fixtures
- read/send multi-target behavior
- rename behavior
- resize planning/convergence
- remap planning matrix
- integration fixtures/sessions
- failure injection
- JSON contract tests
- compatibility tests

Write failing unit/contract tests for stable pure logic before implementing that logic where practical.

For behavior requiring live Zellij, first create a reproducible integration harness and a minimal failing scenario.

## Phase 6 — Implementation

Preferred language: Rust.

Implementation priorities:

1. typed Zellij backend/capability discovery
2. state/domain model
3. selector/target resolution
4. output contracts
5. list/read/send
6. rename/add/remove
7. resize
8. remap
9. completion/docs polish

Do not implement remap as a large command containing ad-hoc subprocess calls. Build on the backend, planner, state, and layout abstractions established earlier.

## Phase 7 — Test and verification

Run:

- formatting/linting
- unit tests
- fake-backend tests
- CLI contract tests
- real Zellij integration tests
- destructive-operation dry-run tests
- remap preservation tests

For remap preservation, prove that the original running process/pane identity is retained in supported scenarios rather than only observing similar screen contents after recreation.

## Phase 8 — Final traceability audit

Update:

```text
docs/design/requirements-traceability.md
```

Every P0 requirement must point to:

- design section
- implementation module/function
- automated test(s)

Classify each requirement as:

```text
implemented
implemented with documented limitation
not implemented
```

No requirement may be marked implemented merely because a lower-level library/backend supports it; the CLI path and tests must exist.

## Phase 9 — User documentation

Deliver:

- README
- examples
- CLI help/completion
- compatibility statement
- known limitations

Examples should emphasize the product's differentiators:

```text
read multiple panes
broadcast input
bulk structural operations
rename
higher-level resize
remap running panes across repeated layout instances
```

Do not lead with trivial replacements for interactive Zellij focus/navigation.

## Coding-agent operating rules

- Research first; do not invent Zellij behavior.
- Prefer official/current Zellij interfaces.
- Do not optimize for implementation simplicity at the expense of the CLI model.
- Do not flatten the CLI into dozens of aliases.
- Do not add a top-level verb just because Zellij has an action of that name.
- Preserve the verb-first design unless a documented exception is clearly superior.
- Keep common usage positional and alternative resolution explicit through options.
- Treat multi-pane operations as set operations with explicit result aggregation.
- Treat `remap` as planning/orchestration, not as `override-layout` renamed.
- Preserve running panes/processes whenever the operation promises preservation.
- Never claim atomicity or exact resize semantics that Zellij cannot guarantee.
- Surface limitations in design, CLI diagnostics, and tests.
