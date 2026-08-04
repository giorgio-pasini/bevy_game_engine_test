# BevyTest documentation

This repository is an experiment: **how well does AI-assisted game development
work with an engine like Bevy, where everything is done in code?**

The games are not the point. They are probes. What is being researched is the
*process* — which practices hold up, which fail, and what it costs.

## The documents

| File | Job | Style |
| --- | --- | --- |
| [journal.md](journal.md) | What happened, dated. The audit trail and the decision record. | Append-only, newest first |
| [ai_gamedev_guide_for_bevy.md](ai_gamedev_guide_for_bevy.md) | What to **do**. Current best understanding. | Prescriptive, terse, scannable |
| [ai_gamedev_antipatterns_for_bevy.md](ai_gamedev_antipatterns_for_bevy.md) | What **not** to do, and why it bites. | Narrative, evidence-first |

The guide and the antipatterns are deliberately written in **different
registers** rather than as mirrors of each other. The guide tells you what to
do; the antipatterns make you believe it by showing what went wrong. If an entry
in one could be pasted into the other unchanged, it is written wrong.

## General vs project-specific

These documents hold **general** findings only. Per-project notes live beside
their code, where an AI session will actually load them:

- `games/snake/CLAUDE.md`
- `games/graphical_adventure_test/DESIGN.md`

### The promotion rule

A finding belongs here only if:

1. It follows from **Bevy, Rust, or the AI workflow itself** — not from one
   game's design, and
2. it survives the question *"would this still be true for a different genre?"*

Everything else stays in the project docs. A project doc that hits a general
rule should **cite** it rather than restate it; duplicated rules drift apart.

## Conventions

**Scope tags.** Every entry carries one, so engine-agnostic findings can be
separated later:

- `[bevy]` — specific to Bevy
- `[rust]` — Rust or cargo generally
- `[tooling]` — build systems, machine limits, environment
- `[ai-workflow]` — working with an AI assistant, engine-independent

**Evidence strength.** This is research with a small sample. Entries are marked:

- `observed once` — happened one time; may be circumstantial
- `confirmed` — seen repeatedly, or follows directly from documented mechanics
- `measured` — has numbers behind it

Re-test `observed once` entries when a new project starts. That is the main way
this documentation improves.

**Version stamps.** Bevy claims are stamped with the version and date verified.
Most Bevy-specific content is true *of a version*, and rot should be visible
rather than silent.

**Numbers over impressions.** Cite the measurement. "Cold build 21m42s, warm
46s" is useful; "builds are slow" is not, and cannot be checked for regression
later.

## Two rules learned by breaking them

**Write the documentation after the work lands, not while it is in flight.**
Twice the docs described work that was still in progress, and once the journal
actively said a feature was outstanding *after it had been built* — which would
have led a future session to build it twice. A document that runs ahead of
reality is worse than no document, because it is trusted.

**Once code exists, the code is authoritative.** Design sketches are provisional
by nature: they are written to decide what to build, and the build teaches you
things. When an implementation diverges from a sketch, do **not** quietly leave
the sketch standing — mark it superseded, say exactly how it differs, and point
at the file that holds the truth.

Keep the original reasoning visible rather than deleting it. A future decision
should be measurable against the argument it overturns, and "we already
considered that, here is why we chose otherwise" is the most valuable thing this
documentation carries.

## Maintaining these

At the end of a working session, update all three:

1. Add a journal entry — what happened, what was learned, what it cost.
2. Promote anything that passes the rule above into the guide or antipatterns.
3. **Edit or delete entries that new evidence contradicts.** The guide is a
   current-state document, not an archive. The journal preserves the history, so
   the guide is free to be rewritten.
4. End the journal entry by noting what it changed in the other two documents.
