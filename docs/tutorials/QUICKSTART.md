# Quickstart: Your First Spec in 20 Minutes

In 20 minutes, you will install xchecker, configure an LLM provider, and
generate a structured spec from a feature idea. By the end you will have
requirements, a design document, and implementation tasks -- all generated from
a one-paragraph problem statement.

---

## Prerequisites

- **Rust 1.89+** installed (`rustc --version`)
- **One LLM provider** configured:
  - Claude CLI (installed and authenticated), OR
  - Gemini CLI (installed with `GEMINI_API_KEY` set), OR
  - OpenRouter account (with `OPENROUTER_API_KEY` set), OR
  - Anthropic API key (with `ANTHROPIC_API_KEY` set)

See [LLM Providers](../guides/LLM_PROVIDERS.md) for provider setup details.

---

## Step 1: Install xchecker (2 min)

```bash
cargo install xchecker
```

Verify the installation:

```bash
xchecker --version
```

---

## Step 2: Check your environment (2 min)

Run the built-in health check:

```bash
xchecker doctor
```

You should see output like:

```
 Claude CLI: found at /usr/local/bin/claude (version 0.8.1)
 Runner: native
 Write permissions: OK
 Configuration: valid
 Atomic rename: supported
```

If a check fails, the doctor output includes suggestions. Common fixes:

- **Claude CLI not found**: Install from https://claude.ai/download, then run
  `claude auth login`.
- **API key not set**: Export the required variable for your provider
  (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, or `GEMINI_API_KEY`).

---

## Step 3: Configure your provider (3 min)

Navigate to your repository root and create a minimal config:

```bash
cd /path/to/your/repo
mkdir -p .xchecker
```

Create `.xchecker/config.toml`:

### Claude CLI (default)

```toml
[llm]
provider = "claude-cli"

[defaults]
model = "sonnet"
```

### Gemini CLI

```toml
[llm]
provider = "gemini-cli"

[llm.gemini]
default_model = "gemini-2.0-flash-lite"
```

### OpenRouter

```toml
[llm]
provider = "openrouter"

[llm.openrouter]
model = "google/gemini-2.0-flash-lite"
budget = 30
```

### Anthropic API

```toml
[llm]
provider = "anthropic"

[llm.anthropic]
model = "sonnet"
```

Also configure which files xchecker should include as context:

```toml
[selectors]
include = [
    "src/**/*.rs",
    "docs/**/*.md",
    "Cargo.toml",
    "*.md",
]
exclude = [
    "target/**",
    ".git/**",
    "node_modules/**",
]
```

---

## Step 4: Generate your first spec (5 min)

Pipe a problem statement into `xchecker spec`:

```bash
echo "Build a user authentication system with OAuth2 support,
including login, logout, and session management with JWT tokens" \
  | xchecker spec my-auth-feature
```

xchecker will:

1. Build a context packet from your selected files
2. Scan the packet for secrets (blocks if any are found)
3. Send the packet and your problem statement to the LLM
4. Write the requirements artifact atomically
5. Generate a receipt with BLAKE3 hashes

You can also create a problem statement file for more detailed input:

```bash
cat > .xchecker/specs/my-auth-feature/context/problem-statement.md << 'EOF'
# User Authentication System

## Goal
Build a secure user authentication system with OAuth2 support.

## Requirements
- User login/logout functionality
- OAuth2 integration (Google, GitHub)
- Session management with JWT tokens
- Password reset flow
EOF

xchecker spec my-auth-feature --source fs
```

**Tip**: Use `--dry-run` to preview what would happen without calling the LLM:

```bash
xchecker spec my-auth-feature --dry-run
```

---

## Step 5: Explore the output (3 min)

Check the spec status:

```bash
xchecker status my-auth-feature
```

Look at the generated artifacts:

```bash
ls .xchecker/specs/my-auth-feature/artifacts/
# 00-requirements.md
# 00-requirements.core.yaml

cat .xchecker/specs/my-auth-feature/artifacts/00-requirements.md
```

Examine the execution receipt:

```bash
ls .xchecker/specs/my-auth-feature/receipts/
```

Each receipt is a JSON file containing the phase name, timestamp, exit code,
BLAKE3 hashes of all outputs, and LLM metadata (provider, model, token counts).

For machine-readable status:

```bash
xchecker status my-auth-feature --json
```

---

## Step 6: Continue the pipeline (5 min)

xchecker runs specs through a sequential phase pipeline:

```
Requirements -> Design -> Tasks -> Review -> Fixup -> Final
```

Continue to the design and tasks phases:

```bash
# Generate design document
xchecker resume my-auth-feature --phase design

# Generate implementation tasks
xchecker resume my-auth-feature --phase tasks
```

Each phase builds on the previous one. The design phase reads the requirements
artifact; the tasks phase reads both requirements and design.

Check overall progress:

```bash
xchecker status my-auth-feature
```

View the generated tasks:

```bash
cat .xchecker/specs/my-auth-feature/artifacts/20-tasks.md
```

You can switch providers per phase if needed:

```bash
xchecker resume my-auth-feature --phase design --llm-provider openrouter
```

---

## What's next

- **[Spec to PR tutorial](SPEC_TO_PR.md)** -- the complete workflow from spec
  through implementation to pull request
- **[Configuration Guide](../guides/CONFIGURATION.md)** -- file selectors,
  hooks, per-phase overrides, and security settings
- **[LLM Providers](../guides/LLM_PROVIDERS.md)** -- detailed provider setup,
  authentication, prompt templates, and cost control
- **[CI Setup](../guides/CI_SETUP.md)** -- enforce spec quality gates in
  CI/CD pipelines

---

## Quick reference

| Command | Description |
|---------|-------------|
| `xchecker doctor` | Check environment health |
| `xchecker spec <id>` | Start spec from problem statement |
| `xchecker resume <id> --phase <phase>` | Continue to a specific phase |
| `xchecker status <id>` | Check spec status |
| `xchecker status <id> --json` | Machine-readable status |
| `xchecker gate <id> --min-phase <phase>` | Validate spec for CI gates |

---

## Troubleshooting

### "Packet overflow"

Your context is too large. Narrow your file selectors:

```toml
[selectors]
include = ["src/auth/**/*.rs"]
```

Or increase the limit:

```bash
xchecker spec my-feature --packet-max-bytes 131072
```

### "Phase timeout"

Increase the timeout:

```bash
xchecker resume my-feature --phase design --phase-timeout 1200
```

### "Claude CLI not found"

Install Claude CLI and authenticate:

```bash
# See https://claude.ai/download for installation
claude auth login
```
