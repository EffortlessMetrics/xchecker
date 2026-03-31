# Claude CLI Stub

The `claude-stub` binary is a test harness that mimics the Claude CLI behavior for testing xchecker without making actual API calls.

## Usage

```bash
cargo run --bin claude-stub -- [OPTIONS]
```

## Options

- `--output-format <FORMAT>`: Output format (stream-json or text) [default: text]
- `--include-partial-messages`: Include partial messages in stream-json output
- `--model <MODEL>`: Model to use [default: haiku]
- `--max-turns <N>`: Maximum number of turns [default: 10]
- `--scenario <SCENARIO>`: Test scenario to simulate [default: success]
- `--no-sleep`: Disable artificial delays (for fast CI tests)

## Test Scenarios

### Success (`--scenario success`)
- **stream-json**: Emits complete stream-json events with realistic requirements phase output
- **text**: Emits plain text requirements document
- **Exit code**: 0

### Partial (`--scenario partial`)
- **stream-json**: Starts normally but interrupts mid-stream
- **text**: Outputs partial content then exits
- **Exit code**: 1

### Malformed (`--scenario malformed`)
- **stream-json**: Emits valid start events then malformed JSON
- **text**: Falls back to success scenario
- **Exit code**: 1

### Text Fallback (`--scenario text-fallback`)
- Always emits malformed JSON to trigger fallback behavior
- **Exit code**: 1

### Error (`--scenario error`)
- Simulates authentication failure
- Outputs error messages to stderr
- **Exit code**: 1

## Examples

```bash
# Test successful requirements generation with stream-json
cargo run --bin claude-stub -- --output-format stream-json --scenario success

# Test error handling
cargo run --bin claude-stub -- --scenario error

# Test malformed JSON handling
cargo run --bin claude-stub -- --output-format stream-json --scenario malformed

# Test partial response handling
cargo run --bin claude-stub -- --output-format stream-json --scenario partial
```

## Output Format

### Stream-JSON Events

The stub emits the following event types in order:
1. `conversation_start` - Conversation initialization
2. `message_start` - Message begins
3. `content_block_start` - Content block begins
4. `content_block_delta` - Content chunks (streamed)
5. `content_block_stop` - Content block ends
6. `message_stop` - Message complete with final metadata

### Requirements Phase Output

The stub generates a realistic requirements document with:
- Proper markdown structure
- User stories in the correct format
- EARS-formatted acceptance criteria
- Multiple requirements with detailed criteria

## Testing

Run the test suite with:

```bash
# Run claude-stub CLI integration tests (requires dev-tools feature)
cargo test --features dev-tools --test claude_stub_cli

# Or run the full test suite
cargo test --all-features
```

The tests validate:
- Version output
- Success scenarios (both text and stream-json)
- Error scenarios
- Malformed JSON handling
- Partial response handling

Note: Tests use `--no-sleep` for fast execution in CI.