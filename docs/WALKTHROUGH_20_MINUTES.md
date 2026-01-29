# Running xchecker on Your Repo in 20 Minutes

This walkthrough guides you through setting up and running xchecker on your existing repository. By the end, you'll have a working spec workflow generating requirements, design, and implementation tasks.

## Prerequisites

Before starting, ensure you have:

- **Rust 1.70+** installed (`rustc --version`)
- **LLM Provider** configured (Claude CLI, Gemini CLI, OpenRouter, or Anthropic API) (see [LLM_PROVIDERS.md](LLM_PROVIDERS.md))
- A repository you want to spec out

## Time Breakdown

| Step | Time | Description |
|------|------|-------------|
| 1. Install | 2 min | Install xchecker |
| 2. Verify | 2 min | Run doctor checks |
| 3. Initialize | 3 min | Set up workspace and spec |
| 4. Configure | 3 min | Customize file selectors |
| 5. Generate | 8 min | Run through phases |
| 6. Review | 2 min | Check outputs |

## Step 1: Install xchecker (2 minutes)

```bash
# From crates.io (recommended)
cargo install xchecker

# Verify installation
xchecker --version
```

## Step 2: Verify Environment (2 minutes)

Run the doctor command to ensure everything is configured correctly:

```bash
xchecker doctor
```

You should see output like:

```
✓ Claude CLI: found at /usr/local/bin/claude (version 0.8.1)
✓ Runner: native
✓ Write permissions: OK
✓ Configuration: valid
✓ Atomic rename: supported
```

If any checks fail, see [DEBUGGING_GUIDE.md](DEBUGGING_GUIDE.md) for solutions.

## Step 3: Initialize Workspace and Spec (3 minutes)

Navigate to your repository root and initialize xchecker:

```bash
cd /path/to/your/repo

# Initialize a workspace (optional but recommended for multi-spec projects)
xchecker project init my-project

# Initialize your first spec
xchecker init my-feature
```

This creates the following structure:

```
your-repo/
├── workspace.yaml           # Workspace registry (if using project init)
└── .xchecker/
    ├── config.toml          # Configuration file
    └── specs/
        └── my-feature/      # Your spec directory
            ├── context/     # Context files for Claude
            ├── artifacts/   # Generated outputs
            └── receipts/    # Execution receipts
```

## Step 4: Configure File Selectors (3 minutes)

Edit `.xchecker/config.toml` to include the files relevant to your project:

```toml
# .xchecker/config.toml

[defaults]
model = "haiku" # or "gemini-2.0-flash-lite", etc.
# See LLM_PROVIDERS.md for provider-specific model names

[selectors]
# Include your source files and documentation
include = [
    "src/**/*.rs",        # Rust source files
    "src/**/*.ts",        # TypeScript files
    "src/**/*.py",        # Python files
    "docs/**/*.md",       # Documentation
    "*.md",               # Root markdown files
    "*.toml",             # Config files
    "*.yaml",             # YAML configs
]

# Exclude build artifacts and dependencies
exclude = [
    "target/**",          # Rust build output
    "node_modules/**",    # Node dependencies
    ".git/**",            # Git internals
    "dist/**",            # Build output
    "__pycache__/**",     # Python cache
    "*.log",              # Log files
]
```

## Step 5: Generate Spec Phases (8 minutes)

### 5.1 Create Problem Statement

First, provide a problem statement for your feature:

```bash
# Option A: From stdin
echo "Build a user authentication system with OAuth2 support, 
including login, logout, and session management" | xchecker spec my-feature

# Option B: Create a problem statement file
cat > .xchecker/specs/my-feature/context/problem-statement.md << 'EOF'
# User Authentication System

## Goal
Build a secure user authentication system with OAuth2 support.

## Requirements
- User login/logout functionality
- OAuth2 integration (Google, GitHub)
- Session management with JWT tokens
- Password reset flow
- Rate limiting for security

## Constraints
- Must integrate with existing user database
- Should support both web and mobile clients
- Must comply with GDPR requirements
EOF

xchecker spec my-feature --source fs
```

### 5.2 Run Through Phases

Now run through each phase:

```bash
# Generate requirements (Phase 1)
xchecker resume my-feature --phase requirements

# Check status
xchecker status my-feature

# Generate design (Phase 2)
xchecker resume my-feature --phase design

# Generate implementation tasks (Phase 3)
xchecker resume my-feature --phase tasks
```

**Tip:** Use `--dry-run` to preview what will happen without making Claude API calls:

```bash
xchecker resume my-feature --phase design --dry-run
```
# Switching LLM Providers

You can easily switch providers using CLI flags or configuration:

```bash
# Use Gemini CLI
xchecker resume my-feature --phase requirements --llm-provider gemini-cli

# Use OpenRouter
xchecker resume my-feature --phase requirements --llm-provider openrouter
```

##
## Step 6: Review Outputs (2 minutes)

Check the generated artifacts:

```bash
# List artifacts
ls -la .xchecker/specs/my-feature/artifacts/

# View requirements
cat .xchecker/specs/my-feature/artifacts/00-requirements.md

# View design
cat .xchecker/specs/my-feature/artifacts/01-design.md

# View tasks
cat .xchecker/specs/my-feature/artifacts/02-tasks.md
```

Check the overall status:

```bash
# Human-readable status
xchecker status my-feature

# JSON status (for automation)
xchecker status my-feature --json | jq .
```

## What's Next?

### Continue Development

Use the generated tasks as your development roadmap:

```bash
# View implementation tasks
cat .xchecker/specs/my-feature/artifacts/02-tasks.md
```

### Set Up CI Gates

Add spec validation to your CI pipeline:

```bash
# Check if spec meets minimum requirements
xchecker gate my-feature --min-phase design

# Fail if spec is stale
xchecker gate my-feature --min-phase tasks --max-phase-age 7d
```

### Integrate with Claude Code

Use xchecker's JSON output for Claude Code integration:

```bash
# Get spec overview
xchecker spec my-feature --json

# Get resume context
xchecker resume my-feature --phase design --json
```

See [CLAUDE_CODE_INTEGRATION.md](CLAUDE_CODE_INTEGRATION.md) for detailed integration flows.

## Quick Reference

| Command | Description |
|---------|-------------|
| `xchecker doctor` | Check environment health |
| `xchecker init <spec>` | Initialize a new spec |
| `xchecker spec <spec>` | Start spec from problem statement |
| `xchecker resume <spec> --phase <phase>` | Resume from a specific phase |
| `xchecker status <spec>` | Check spec status |
| `xchecker status <spec> --json` | Get JSON status |
| `xchecker gate <spec> --min-phase <phase>` | Validate spec for CI |
| `xchecker clean <spec>` | Clean up spec artifacts |

## Troubleshooting

### "Claude CLI not found"

Install and authenticate Claude CLI:

```bash
# Install Claude CLI (see official docs)
# Then authenticate
claude auth login
```

### "Packet overflow"

Your context is too large. Adjust selectors:

```toml
[selectors]
# Be more specific about what to include
include = ["src/auth/**/*.rs"]  # Only auth module
exclude = ["**/*.test.rs"]       # Exclude tests
```

Or increase limits:

```bash
xchecker spec my-feature --packet-max-bytes 131072
```

### "Phase timeout"

Increase the timeout:

```bash
xchecker resume my-feature --phase design --phase-timeout 1200
```

## See Also

- [CONFIGURATION.md](CONFIGURATION.md) - Full configuration reference
- [CLAUDE_CODE_INTEGRATION.md](CLAUDE_CODE_INTEGRATION.md) - Claude Code integration
- [DEBUGGING_GUIDE.md](DEBUGGING_GUIDE.md) - Troubleshooting and diagnostics
- [LLM_PROVIDERS.md](LLM_PROVIDERS.md) - LLM provider configuration
