# zelper Requirements Specification

Status: Basic requirements for detailed design and implementation

## 1. Purpose

Zellij exposes many useful actions and state-inspection commands, but using them for external automation requires remembering unrelated command names, manually resolving pane/tab IDs, switching between representations, and composing multiple low-level actions for operations that are conceptually one task.

`zelper` SHALL provide a coherent automation and orchestration CLI over Zellij.

Its main value is not replacing interactive Zellij keybindings. Its main value is to make operations involving **multiple panes/tabs, external scripts, coding agents, layout changes, and repeatable workspace manipulation** simple and predictable.

## 2. Primary use cases

The product SHALL support the following use cases as first-class concerns.

### 2.1 Inspect Zellij structure

A user can inspect:

- sessions
- tabs
- panes
- layouts available by name
- pane/tab identity and relevant metadata
- current geometry / layout-relevant state where available

Machine-readable output SHALL be available for automation.

### 2.2 Read pane output

The user can retrieve visible output or scrollback from one or more panes without manually focusing each pane.

Requirements:

- single-pane read
- multi-pane read
- readable boundaries/metadata in human output
- structured machine-readable output
- explicit control of visible-screen vs full scrollback where Zellij supports it
- failures for one pane should be identifiable when a multi-pane operation is performed

### 2.3 Send input to panes

The user can send text and/or keys to one or more panes.

Requirements:

- text without implicit Enter
- explicit Enter behavior
- key/control-character support where feasible through Zellij
- multi-pane/broadcast operation
- per-target success/failure reporting
- unsafe ambiguity SHALL produce an error rather than selecting an arbitrary target

This is an important coding-agent management use case, eg. sending `y` to several waiting panes after a rate-limit window.

### 2.4 Rename panes and tabs

Rename is a first-class wrapper operation because performing it programmatically through raw Zellij actions is unnecessarily cumbersome.

Requirements:

- rename a pane
- rename a tab
- support multiple targets where useful
- provide a path for generated/pattern-based names in detailed design if this can remain understandable and safe

### 2.5 Resize panes

Resize SHALL be intent-oriented rather than merely exposing Zellij's directional resize action.

At minimum, detailed design must investigate and support useful higher-level operations such as:

- resizing a specific pane
- equalizing a set of panes
- assigning useful relative/percentage sizing when this can be reliably mapped to Zellij behavior
- resizing a selected tab's pane set

Exact syntax and achievable guarantees are a detailed-design task and must be validated against the installed/current Zellij behavior.

### 2.6 Remap existing panes into another layout

`remap` is a core feature.

The conceptual operation is:

> Preserve existing pane contents/processes and reorganize those panes according to a selected layout. If the number of panes exceeds one layout instance, create/use additional tabs and apply the layout repeatedly until all panes are placed.

Requirements:

- existing terminal panes SHALL be preserved wherever Zellij permits this
- remap SHALL NOT be implemented as killing panes and recreating their commands unless the user explicitly chooses a destructive/recreate mode introduced by future design
- the default layout source is a **layout name**
- path or inline definitions are secondary/explicit alternatives, not overloaded positional syntax
- overflow behavior SHALL be deterministic
- default overflow model to investigate/design: repeat the layout across additional tabs until all selected panes are assigned
- if a layout has `N` usable slots and there are `M > N` selected panes, additional layout instances SHALL be created/used as needed
- tab/window naming and reuse policy must be specified in detailed design
- pane ordering for slot assignment must be deterministic and documented
- remap scope must support at least the current tab; session/multi-tab scope should be supported if technically sound
- plugin panes must be treated explicitly; they must not accidentally be destroyed or silently mixed into terminal-pane semantics
- a dry-run/plan output is strongly required before potentially large remaps

Zellij already supports runtime `override-layout`, retaining existing terminal panes, active-tab-only application, raw inline layouts, and swap layouts. Detailed design SHALL assess how much of `remap` can use these primitives and where orchestration is still required.

### 2.7 Add panes and tabs

The user can add workspace capacity through `zelper`.

Requirements:

- add pane(s)
- add tab(s)
- count > 1 should be supported where useful
- allow creation associated with a named layout where appropriate
- creation should be usable by remap internally

### 2.8 Remove panes and tabs

The user can remove panes/tabs individually or as selected sets.

Requirements:

- remove pane(s)
- remove tab(s)
- useful structural cleanup such as removal of empty tabs should be considered in detailed design
- destructive multi-target actions require clear preview/confirmation policy or explicit noninteractive flags suitable for scripting

### 2.9 Layout resolution

Layouts SHALL have a consistent resolution model.

Primary/common form:

```text
zelper remap NAME
```

Secondary alternatives SHALL be explicit options, conceptually similar to:

```text
zelper remap --path PATH
zelper remap --inline SPEC
```

The exact option names may change during detailed design if there is a clear improvement, but the semantic rule is mandatory:

- bare positional operand = normal/default resource form
- alternative source forms = explicit options
- mutually exclusive source forms must be rejected when supplied together

The same principle applies throughout the CLI, not only to `remap`.

## 3. CLI design requirements

### 3.1 Verb-first structure

Top-level arguments SHOULD be verbs.

Examples of the intended grammar style, not a frozen command list:

```text
zelper read ...
zelper send ...
zelper rename ...
zelper resize ...
zelper remap ...
zelper add ...
zelper remove ...
zelper list ...
```

Do NOT mechanically expose every Zellij action as a top-level command.

### 3.2 Small, structured top-level vocabulary

A large flat list of unrelated commands is unacceptable.

Detailed design SHALL:

- minimize the number of top-level verbs
- group related variants under a coherent grammar
- avoid synonyms (`show`/`info`/`inspect` etc.) unless they have intentionally different semantics
- provide shell completion so memorization is not the primary discovery mechanism

### 3.3 Positional-first, explicit alternatives

For every command:

- the most common, semantically primary operand form should be positional
- alternate resolution modes, filtering, bulk selection, source overrides, and policy changes should use named options
- do not encode multiple unrelated mini-languages into the same positional argument

For example, the design should prefer an approach conceptually like:

```text
zelper read 12
zelper read --tab agents
zelper read --command codex
```

rather than forcing users to memorize positional selector prefixes such as `pane:12`, `tab:agents`, `cmd:codex` as the main interface.

An advanced query/filter expression may exist later as an escape hatch, but not as the primary grammar.

### 3.4 Consistent target options

The same target concept SHALL use the same option name across commands wherever practical.

Candidate shared target dimensions include:

- ID
- pane/tab name
- tab
- command
- cwd
- all
- current/focused context where needed

The detailed design SHALL determine the canonical option names and collision/ambiguity rules.

### 3.5 Multi-target semantics

Operations that naturally apply to sets SHALL treat multi-target behavior as a first-class feature, not an afterthought.

In particular:

- `read`
- `send`
- `rename` where meaningful
- `resize`
- `remove`
- `remap`

must define deterministic iteration, error aggregation, output, and exit status semantics.

### 3.6 Human and machine output

Commands that return data SHALL support:

- concise human-readable output by default
- stable JSON output for scripts/agents

The detailed design SHALL define a versionable JSON schema or at least stable typed output structures before implementation.

### 3.7 Dry run

Operations that can make broad structural/destructive changes SHOULD support `--dry-run`, especially:

- remap
- resize of multiple panes
- bulk rename
- bulk remove

Dry run must report the resolved targets and planned operations without mutating Zellij state.

## 4. Scope and prioritization

### P0 — central value

- robust Zellij state discovery
- pane/tab IDs and metadata
- layout discovery/resolution
- read one/many panes
- send one/many panes
- rename pane/tab
- add/remove pane/tab
- resize at useful wrapper-level semantics
- remap with preservation and repeated-layout overflow
- JSON output
- shell completion
- clear error handling
- dry-run for broad mutations

### P1 — desirable after the core is correct

- richer selection filters
- saved groups / aliases for pane sets
- watch/wait semantics for pane output or state
- batch execution policies
- richer remap policies (reuse tabs, naming policies, overflow variants)
- configuration file for defaults

### P2 — not required for the first implementation

- TUI
- daemon/background service
- replacement for Zellij's normal interactive navigation
- exhaustive wrappers for every Zellij action

## 5. Explicit non-goals

The first version SHALL NOT aim to:

- replace Zellij itself
- duplicate every interactive keybinding
- expose a one-to-one alias for every `zellij action`
- build a general terminal multiplexer abstraction supporting tmux/screen
- recreate panes just to perform remapping when preservation is possible
- invent a large selector DSL before common positional/options grammar is proven insufficient

Commands such as focus/move that are already easy interactively are lower priority unless they are needed as internal primitives or become valuable as part of multi-step wrapper operations.

## 6. Technology requirements

The implementation is an independent CLI program, not a shell alias/function collection.

Rust is the preferred implementation language unless research identifies a material reason to choose another language.

Implementation complexity is **not** a reason to select a weaker design; a coding agent will perform the implementation.

If Rust is used, the detailed design should evaluate mature libraries for:

- CLI parsing/subcommands/completion
- serialization
- subprocess execution
- KDL parsing if native parsing is necessary
- diagnostics/errors
- table/human output
- integration/CLI testing

Library complexity by itself is not a concern.

## 7. Compatibility and dependency on Zellij

`zelper` may invoke the installed `zellij` executable as its primary backend.

Detailed design SHALL determine:

- minimum supported Zellij version
- feature detection/version detection strategy
- whether to parse Zellij JSON output or other structured output
- behavior when a required Zellij feature is unavailable
- whether any operation requires direct interaction with Zellij IPC/state beyond the public CLI; such access must be justified and isolated

Prefer public/stable Zellij interfaces over internal state formats.

## 8. Safety and transactional behavior

Structural operations can partially fail. Detailed design SHALL define:

- preflight validation
- target snapshots before mutation
- operation planning
- deterministic operation ordering
- partial-failure reporting
- rollback feasibility per operation
- best-effort recovery where atomic rollback is impossible

`remap` requires particular attention. The implementation must not claim atomicity if Zellij cannot provide it.

## 9. Acceptance criteria

The initial implementation is acceptable only when all of the following are demonstrated by automated tests and/or reproducible integration tests:

1. A user can discover sessions/tabs/panes and IDs without raw Zellij command knowledge.
2. A user can read one pane and multiple panes.
3. A user can send input to one pane and broadcast to multiple panes.
4. A user can rename both a pane and a tab.
5. A user can perform at least one higher-level resize operation that would otherwise require multiple low-level actions.
6. A user can add and remove panes/tabs through the wrapper.
7. A user can remap existing running terminal panes into a named layout without killing/recreating those panes in the supported case.
8. When pane count exceeds one layout instance, remap deterministically uses further tab/layout instances according to the documented overflow policy.
9. Layout path and inline forms are available only as explicit alternative options, not ambiguous positional overloads.
10. Multi-target operations report per-target failure clearly.
11. Relevant query commands have stable JSON output.
12. Broad mutations have a non-mutating preview/dry-run path.
13. Shell completion is generated/provided for the supported shells selected by detailed design.
14. Help text exposes the command structure well enough that the user need not memorize a flat command catalog.

## 10. Open questions delegated to detailed design

The coding agent must research and resolve, documenting rationale:

- Exact canonical top-level verb set.
- Whether `list` is top-level or whether a better very-small discovery structure exists.
- Exact target option grammar and precedence.
- Numeric pane/tab ID representation across Zellij versions.
- Pane ordering used by remap (visual order, Zellij order, explicit order, etc.).
- How to move existing panes across tabs using public Zellij capabilities, and what limitations exist.
- Whether runtime override alone can implement desired remap behavior or whether coordinated pane moves/new tabs are required.
- Handling of floating panes, stacked panes, and plugin panes.
- Percentage/equal resize feasibility and convergence strategy.
- Definition of an "empty" tab for cleanup.
- Confirmation behavior for destructive bulk actions in interactive and noninteractive environments.
- Stable JSON schema versioning strategy.
- Configuration and layout registry resolution order.

Do not guess these into implementation. Research, prototype where needed, then freeze them in detailed design.

## 11. Reference facts from current Zellij documentation

At the time this requirements document was prepared (2026-08-21), Zellij documentation describes:

- layouts in KDL and named layouts in the default layouts directory
- runtime `override-layout`
- retaining existing terminal panes during override
- applying override only to the active tab
- inline raw KDL layout strings
- swap layouts that rearrange panes according to pane-count constraints
- session resurrection with serialized session layout/state

The implementation must verify the exact installed/current CLI syntax during research rather than treating examples here as an eternal API contract.

Official documentation references:

- https://zellij.dev/documentation/cli-actions.html
- https://zellij.dev/documentation/layouts.html
- https://zellij.dev/documentation/swap-layouts.html
- https://zellij.dev/documentation/cli-recipes.html
- https://zellij.dev/documentation/session-resurrection.html
