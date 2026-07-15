# crucible

**Put your code in, find out what it's really made of.**

A deterministic engine that measures the *simplicity* of Rust code — honestly, by
orthogonal primitives, and ranked against your own codebase rather than against
invented thresholds it could never defend.

The one-line thesis: **a truth instrument may only point at a fact it can measure.**
So crucible measures the axes of "simple" that are actually there in the syntax tree
— branching, depth, size, state, density — and it never emits a verdict it can't back.
There is no "quality: 7/10" here, because that number would be a guess wearing a
decimal point. There is only *"this function is in the top 1% most complex in your
repo, here is the number, here is the span."*

## What it does not do (on purpose)

- It does not grade. It **ranks, relative to your codebase** — no universal "complexity
  10 = bad" threshold, because none exists honestly. What's fine for a parser is sloppy
  for a config loader, and knowing which is which needs *context crucible does not have
  yet* (see below).
- It does not average. A file of 40 trivial functions and one monster does not come out
  "fine" — the monster is the story, and a mean would hide it.
- It makes **zero model calls.** Every number is reproducible and traces to a source
  span.

## The road it's on

crucible is the *thick engine* of a larger idea. Today it measures. Later — its own
dedicated arc, not this one — a small, glass-box model (built on
[eyeofrah](../eyeofrah)) will read these measurements as a *sense*, classify each unit's
**role**, and select a yardstick fair to that role — a parser judged as a parser. That
model is "the fused limb." crucible is built model-agnostic so the limb drops on later
without rework: the role/yardstick column simply sits empty until it arrives.

Status: design (see `docs/superpowers/specs/`). v1 is the Simplicity dimension, Rust
only, CLI report, dogfooded on real code.
