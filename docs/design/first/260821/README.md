# zelper — Coding Agent Handoff

`zelper` is a structured automation/orchestration CLI for Zellij.

This directory is the handoff package for the coding agent that will perform research, detailed design, implementation, and testing.

## Documents

1. `requirements.md` — product requirements and acceptance criteria
2. `basic-design.md` — architectural and CLI basic design
3. `development-plan.md` — required research/design/review/test/implementation workflow

## Product definition in one sentence

`zelper` provides a consistent, automation-oriented command layer over Zellij for inspecting, reading, sending to, renaming, resizing, adding/removing, and especially remapping multiple existing panes/tabs while preserving their running contents/processes whenever the requested operation permits it.

## Important design direction

This is **not** a collection of aliases and must not be designed as a one-to-one shortening of `zellij action ...` commands.

The public interface must represent user intent at a higher level than Zellij's low-level actions. A single `zelper` command may query state and invoke multiple Zellij operations.

The CLI executable name is:

```text
zelper
```

Top-level commands should normally begin with a **verb**. A very small number of high-frequency or structurally exceptional commands may be exceptions when detailed design provides a strong reason.

## Authority

When documents differ, use this precedence:

1. `requirements.md`
2. `basic-design.md`
3. `development-plan.md`
4. implementation convenience

Do not silently weaken requirements to simplify implementation.
