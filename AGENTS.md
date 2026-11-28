# AGENTS.md — Silvestris

Rules for agent sessions working on Silvestris. Read [README.md](README.md), [ROADMAP.md](ROADMAP.md) and the
relevant `docs/*.md` before touching a subsystem.

## What Silvestris is

Silvestris is the GUI layer for lotus, sandcat, chaus and johjoh, styled after Mac OS X 10.6 Aqua (plus designed
Dark Aqua and Geist appearances). Apps compose and extend it. They do not paint their own controls. Every
bespoke widget in a consumer is either a Silvestris gap (tracked in the ROADMAP gap register) or a deliberate
app-owned domain view. Three reference apps (Finder #49, Installer #31, System Preferences #56) are the
acceptance tests: if one needs a private widget, Silvestris is missing a component.

Sandcat currently builds against an uncommitted checkout. Its adoption is deferred until Silvestris's own
foundations land. It is not a gating consumer: do not add provisional APIs, keep dirty-tree components or
shape the roadmap around it. It re-pins afterwards.

## READ BEFORE WRITING CODE

Read **all** of the general standard and the Rust guide before writing or revising code:

- Windows: `~\workdir\style-guides\README.md` and `~\workdir\style-guides\rust-style-guide.md`
- Unix-like: `~/workdir/style-guides/README.md` and `~/workdir/style-guides/rust-style-guide.md`

If that checkout is absent, read the mirror at <https://github.com/estyseesghosts/style-guides>. Markdown docs
follow `markdown-style-guide.md`. Put tests in their own files.

## READ THE CORPUS

Visual and behavioural decisions come from the hig-09 research wiki, locally at `~/workdir/hig-09`
(<https://github.com/estyseesghosts/hig-09>):

- `03-aqua-interface/*.md`, `02-macintosh-experience/*.md`: HIG 2009 notes with page references.
- `specs/*.md`: testable component specs. `notes/component-inventory.md`, `notes/accessibility-checklist.md`.
- `vm-screenshots/SL/`, `vm-screenshots/*.png`, `images/`: Mac OS X 10.6 captures. These are the reference for
  geometry, materials and states.
- `Apple-OSXHIGuidelines-2009.pdf`: the source. Cite page numbers.

Measured tokens live in `reference/measurements.json`; tools in `tools/metrology/` and `tools/capture/`. Anything
not measured is an estimate and goes in `docs/exceptions.md`.

Do not edit hig-09 from a Silvestris task. Corpus fixes are separate hig-09 commits; tell the user.

## Truth and Citations

You are encouraged to come up with creative solutions for the obscure and arcane problems you might face while working in this folder. This does not mean you just throw shit at the wall to see what sticks. Every decision you make, every change you write, you must be able to justify that decision against a citation - either a line from a source file, a HIG page, a corpus capture, the project documentation provided to you, or otherwise. Do not pull shit out of your ass.

## Half Steps

Having a task's deliverables fully achieved on paper does not always mean those deliverables were completed correctly. A control that renders but ignores focus, disabled and inactive-window states, or a component that works in the gallery but forces consumers to reimplement half of it, is not done. Do not find the minimum possible way to complete a deliverable if it means compromising on that deliverable, or giving the user something similar to - but not exactly what they asked for.

## Writing Style Rules

These rules apply to both chat responses, and to written documentation.

- **Be direct.** Answer the question immediately. No introductions, filler, or unnecessary conclusions.
- **Use natural language.** Write like a competent engineer, not a marketer, consultant, or chatbot.
- **Be concise, not shallow.** Keep explanations short where possible, but never sacrifice technical detail, accuracy, or important nuance.
- **Avoid AI-sounding prose.** No artificial contrasts, dramatic fragments, forced profundity, rhetorical questions, or rules of three.
- **Don't flatter or validate.** Skip praise, excessive agreement, performative empathy, and conversational pleasantries.
- **Be specific.** Prefer concrete facts, mechanisms, measurements, examples, and actionable steps over vague abstractions.
- **Don't overstructure.** Use headings, lists, and tables only when they genuinely improve readability.
- **Be honest about uncertainty.** Distinguish verified facts, reasonable inferences, and speculation. Don't manufacture confidence.
- **Think critically.** Challenge incorrect assumptions, identify tradeoffs, and explain disagreements without hedging unnecessarily.
- **Prioritize substance.** Every sentence should contribute useful information. If removing it changes nothing, remove it.

## Tracker and roadmap

- Work from the tracker: epic #72, milestones M0–M7. Respect issue dependencies; do not start an issue whose
  dependencies are open without saying why.
- Every issue has Why, References, Deliverables, Avoid, Regression traps, Done when. New issues use the same
  anatomy (`.github/ISSUE_TEMPLATE/component.yml`) and get a milestone, layer label, priority and consumer labels.
- A gap found mid-task becomes a new issue, not an unscoped addition to the current one.
- Update `docs/coverage.md` and the ROADMAP gap register when a component or consumer gap changes.

## Layering

- **Silvestris owns** generic controls, views, windows, chrome, themes, command routing, focus, modality,
  transient surfaces, the text-editing core and preference *widgets*.
- **Consumers own** domain models, sessions, persistence locations, file formats and app policy (lotus tiling and
  guest input rules, sandcat sessions, chaus's Ropey editor and Vim, johjoh packaging).
- **GPUI owns** platform paint and input. Build on its focus handles, actions and input handlers; do not replace them.
- One mechanism per job. One command system (`CommandRegistry` + responder chain), one text engine, one
  transient-surface manager, one scroller painter with adapters, one divider implementation.

## Explicitly forbidden shortcuts

- **Do not** add consumer names, types or policies to Silvestris crates.
- **Do not** build a second version of something Silvestris already has. Extend it or file an issue.
- **Do not** land `TextArea` in its current prototype form. #58 owns the multiline text API and uses the
  prototype's duplication inventory as its checklist.
- **Do not** extend code an open issue is replacing (`TextArea` before #58, `ConfirmationSheet` before #21/#22).
- **Do not** take geometry from memory or from Mac OS X 10.7+ (square buttons, overlay scrollers, flat materials).
- **Do not** change shared tokens to fix one control.
- **Do not** branch interaction code on theme. Themes differ in tokens and materials only.
- **Do not** let a control own its own outside-click, Escape or z-order handling. Use the transient layer.
- **Do not** copy, trace or recolour Apple artwork, or commit corpus captures or crops to this public repo.
  Commit hashes and rectangles only.
- **Do not** land visual changes without goldens, or behavioural changes without scenario tests.
- **Do not** mix rustfmt changes with behaviour changes in one commit.
- **Do not** fix a Silvestris bug inside a consumer, or ask a consumer to vendor or patch Silvestris.
- **Do not** change the GPUI revision or Rust toolchain without a note in `docs/consumers.md` and a consumer
  build check.
- **Do not** sleep on wall-clock time in tests or key tests on index-generated element ids.

## Regression traps

Check these whenever focus, input, windows, lists or persistence change:

- lotus guest input isolation: shell shortcuts never reach the guest; guest keys never trigger shell commands.
- lotus inspector focus deferral (lotus#74), Settings bounds restore, `desktop.json` `schema_version=1` and
  unknown-field round-trips.
- Programmatic selection and value updates must not fire user callbacks.
- sandcat composer: Enter submits, Shift+Enter inserts a newline, drafts survive failed submits; Ctrl+1–4 do
  nothing while it has focus; the transcript keeps following the bottom while streaming.
- Closing a window, entity or document mid-dispatch, with a sheet open, or during a save.
- IME composition across undo and focus changes; graphemes and surrogate pairs; password fields leaking through
  copy, accessibility or IME queries.
- Selection and scroll preserved by id across sort, reload and live updates.
- Inactive-window appearance, all four appearances, and 100/125/150/200% scale.
- Idle repaint: blinking, pulsing and animated controls repaint only themselves and stop when the window is
  inactive.

## Build and verify

Windows uses the GNU Rust host. GPUI is pinned to `6b720973027999bae402df23d469026f4cced988`, Rust to `1.98.1`;
check `Cargo.toml` and `rust-toolchain.toml` rather than assuming.

Rust formatting follows `~/workdir/style-guides/rust-style-guide.md`: `rustfmt.toml` sets `max_width = 100`.
`hex_literal_case` and `format_macro_matchers` are unstable on the pinned stable toolchain, so they are
omitted per the style guide's own exception. `#[rustfmt::skip]` is only for manual alignment or generated
code. A format-only commit must contain no manual edits; `git diff -w` will still show rustfmt's trailing
commas, so check the non-comma hunks instead of expecting an empty diff.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p silvestris-demo --bin gallery --locked
```

Window captures: `tools/capture/capture-window.ps1 -ExactTitle … -WindowOnly`. Headless tests prove GPUI dispatch,
not native input, OS window behaviour or font rasterisation. Do not claim more.

## Completion definition

An issue is done when the definition of done in ROADMAP.md holds: gates pass; behaviour has tests; visual work has
goldens in all four appearances at the required scales and cites its corpus crops; the gallery shows every new
public type and state; `docs/coverage.md` and rustdoc are updated; no consumer-specific code was added; and a
consumer or reference app uses the API, or an adoption issue is filed in that consumer's repo.

## Delegating to subagents

A lead agent must give every subagent it calls **explicit instructions, explicit deliverables and explicit avoidables**. Every brief states:

1. **Objective**: the one bounded slice and why it exists, with its issue number.
2. **Allowed paths**: the exact files or directories the subagent may read and edit.
3. **Deliverables**: code, tests, goldens, docs, and the report format (files changed, commands run, results, open questions).
4. **Acceptance checks**: the commands or observations that prove the work is done, and the expected result.
5. **Avoidables**: paths outside the slice, unrelated refactors, consumer repos, hig-09, commits/pushes unless assigned, and the issue's own Avoid and Regression traps.
6. **Stopping point**: when to stop and report instead of guessing (the slice grows, the brief conflicts with this file or the issue, or a check cannot pass).

If the lead cannot fill in all six, the slice is not ready to delegate.

## Git identity

The only git identity an agent may use in this repository is:

```text
terezi <jules@foxtails.me>
```

Set it in the repository itself before the first commit, and do not rely on a
machine-wide default or invent a placeholder:

```sh
git config user.name terezi
git config user.email jules@foxtails.me
```

Every agent commit carries both fields. Do not use `A4`,
`a4@example.invalid`, the host's auto-detected address, or any other
identity, and do not author on someone else's behalf. Do not add
`Co-Authored-By` or any other agent attribution to commits or PRs. If a
commit already exists under another identity, report it to the lead rather
than rewriting history.
