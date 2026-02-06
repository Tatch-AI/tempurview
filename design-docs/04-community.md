---
marp: true
theme: ember
paginate: true
---

<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: true, theme: 'dark', flowchart: { htmlLabels: true, nodeSpacing: 30, rankSpacing: 30 } });
</script>

# Community & Extensibility

### Growing TemPurview as an open-source project

---

## Extensibility: The Core Interface

The `TemporalClient` trait is the primary extension point:

```rust
#[async_trait]
pub trait TemporalClient: Send + Sync {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64>;
    async fn list(&self, filter: &WorkflowFilter, limit: u32)
        -> ClientResult<Vec<WorkflowSummary>>;
    async fn describe(&self, id: &str, run_id: Option<&str>)
        -> ClientResult<WorkflowDetail>;
    async fn get_history(&self, id: &str, run_id: Option<&str>)
        -> ClientResult<Vec<HistoryEvent>>;
    async fn cancel(&self, id: &str, run_id: Option<&str>) -> ClientResult<()>;
    async fn terminate(&self, id: &str, run_id: Option<&str>, reason: &str)
        -> ClientResult<()>;
}
```

Three implementations today: **Grpc**, **Mock**, **Cli**.
Anyone can add a fourth:

- **CachedTemporalClient** — wraps Grpc with an in-memory or Redis cache for repeated reads
- **ReplayTemporalClient** — serves from recorded JSON fixtures for offline demos and CI
- **MultiClusterClient** — fans out to multiple Temporal clusters, merges results

---

## Extensibility: Where to Plug In

| Extension Point | What It Enables |
|----------------|-----------------|
| `TemporalClient` trait | New backends (e.g., Temporal Cloud API, cached proxy) |
| `TableDisplay` trait | New output formats for any domain type |
| `Commands` enum | New CLI subcommands (add variant + handler) |
| `View` enum | New TUI views (follow the checklist) |
| Insight algorithms | New finding types in `insights_compute.rs` |
| Web API handlers | New `/api/*` endpoints in `src/web/` |

**Design principle**: every extension follows an existing pattern.
No framework. No plugin API. Just traits, enums, and match arms.

---

## Extensibility: JSON as the Universal Interface

External tools don't need to modify TemPurview at all:

```bash
# Custom alerting
tpv insight scan -o json | my-alerter

# Custom dashboards
tpv workflow list -o json | jq '...' | my-dashboard

# LLM agents
tpv workflow list --status failed -o json | llm-agent
```

The CLI's JSON output is a **stable contract**.
Every domain type derives `Serialize`.
Pipe it wherever you want.

---

## Contributing

**Development setup:**

```bash
git clone https://github.com/Tatch-AI/tempurview
cd tempurview
cargo build                    # build
cargo test                     # run 152+ unit tests
tpv --mock                     # run TUI with mock data
tpv serve --mock               # run web UI with mock data
```

**Pre-commit hooks** via prek:

```bash
cargo install prek             # one-time
prek install                   # installs hooks
```

- `cargo fmt` — on every commit
- `cargo clippy` — on every commit
- `cargo test` — on every push

---

## Contributing: PR Workflow

<div class="mermaid">
graph LR
    A["Branch"] --> B["Write code"]
    B --> C["prek runs fmt<br>+ clippy"]
    C --> D["Open PR"]
    D --> E["CI: build + test<br>+ publish dry-run"]
    E --> F["Review<br>(1 approver required)"]
    F --> G["Merge to main"]
    G --> H["autorel:<br>tag + release"]
    H --> I["Publish to<br>crates.io"]
    style A fill:#295264,stroke:#5097b7,color:#f6e1ce
    style B fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style C fill:#295264,stroke:#5097b7,color:#f6e1ce
    style D fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style E fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style F fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style G fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style H fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style I fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
</div>

`main` is **protected** — no direct pushes.
All changes go through PRs with at least one approval.

Conventional commits drive **automatic semver**:
`feat:` → minor, `fix:` → patch, `feat!:` → major.

---

## Documentation Site

**mdBook** at [tatch-ai.github.io/tempurview](https://tatch-ai.github.io/tempurview/)

```
docs/src/
  getting-started/
    installation.md         cargo install, from source
    configuration.md        env vars, flags, config file
  cli/
    overview.md             all commands at a glance
    workflows.md            workflow list/get/count/cancel/terminate
    activities-events.md    activity list, event list
    insights.md             insight scan
  tui/
    overview.md             TUI introduction
    navigation.md           views, navigation, input modes
    views.md                each view explained
    keybindings.md          full keybinding reference
```

Auto-deployed to **GitHub Pages** on push to `docs/**`.

---

## Continuous Integration

Three GitHub Actions workflows:

| Workflow | Trigger | Steps |
|----------|---------|-------|
| **CI** | Push to main, all PRs | `cargo build` → `cargo test --lib` → `cargo publish --dry-run` |
| **Release** | Push to main | autorel (semver from commits) → git tag → crates.io publish |
| **Docs** | Push to `docs/**` | mdBook build → GitHub Pages deploy |

**Key design choice**: CI does **not** check out the proto submodule.
This validates the same build path as `cargo install tempurview` —
pre-generated proto code only, zero protoc dependency.

---

<!-- _class: compact -->

## CI: What Each Workflow Validates

**CI** (every PR):
- Compilation succeeds without proto submodule
- 152+ unit tests pass
- `cargo publish --dry-run` succeeds (validates crate metadata)

**Release** (main only):
- Runs `cargo test --lib` as pre-release gate
- autorel parses conventional commits for version bump
- Only publishes when a new tag is created
- `sed` patches `Cargo.toml` version from git tag at publish time

**Docs** (docs changes only):
- Path-filtered: only triggers on `docs/**` changes
- Concurrency group prevents parallel deploys
- Uses GitHub Pages environment for deploy URL

---

## Issue & Feature Request Tracking

**GitHub Issues** with structured templates:

| Template | Fields |
|----------|--------|
| Bug Report | Version, OS, steps to reproduce, expected vs actual, logs |
| Feature Request | Use case, proposed solution, alternatives considered |
| Question | Context, what you tried, relevant docs |

**Labels** for triage:

| Label | Meaning |
|-------|---------|
| `bug` | Something isn't working |
| `enhancement` | New feature or improvement |
| `good first issue` | Accessible entry point for new contributors |
| `help wanted` | Maintainer needs community help |
| `cli` / `tui` / `web` | Affected surface area |
| `insights` | Related to the insights engine |

---

## Issue Tracking: Why Not Discussions?

**GitHub Issues** over Discussions because:

- Issues have **labels, milestones, and assignees** — Discussions don't
- Issues integrate with **PR references** (`Fixes #123`)
- Issues appear in **project boards** for planning
- TemPurview is a tool, not a community platform — most interactions will be bug reports and feature requests

**Discussions** may make sense later for:
- Architecture RFCs
- "Show and tell" (custom scripts, integrations)
- General Q&A that isn't a bug or feature request

Start with Issues. Add Discussions when the community asks for it.

---

## Summary

| Area | Approach |
|------|----------|
| Extensibility | Traits + enums, not plugins. JSON output for external tools |
| Contributing | Fork → PR → CI gate → autorel. Pre-commit hooks for local quality |
| Documentation | mdBook on GitHub Pages, auto-deployed |
| CI/CD | 3 workflows: test, release, docs. Conventional commits → semver |
| Issue tracking | GitHub Issues with templates. Labels for triage |

**Philosophy**: use GitHub's built-in tools.
No external services. No custom infrastructure.
The same principle as the CLI itself — composable, standard, boring.
