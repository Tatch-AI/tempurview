# CLI Overview

Run `tpv` (or `tempurview`) with a subcommand to use the CLI. Without a subcommand, the interactive TUI launches instead.

```bash
tpv <command> [options]
```

## Commands

| Command | Description |
|---------|-------------|
| `workflow list` | List workflows with optional filters |
| `workflow get <id>` | Get details of a specific workflow |
| `workflow count` | Count workflows matching a filter |
| `workflow cancel <id>` | Cancel a running workflow |
| `workflow terminate <id>` | Terminate a workflow |
| `activity list <id>` | List activities for a workflow |
| `event list <id>` | List history events for a workflow |
| `insight scan` | Scan workflows for operational insights |
| `config show` | Show resolved configuration |
| `config profile-add <name>` | Add a connection profile |
| `config profile-list` | List all connection profiles |
| `config profile-remove <name>` | Remove a connection profile |
| `config set-default <name>` | Set the default profile |
| `test-connection` | Test connection to Temporal server |

## Output format

By default, output is auto-detected:
- **Table** when writing to a terminal (TTY)
- **JSON** when piped to another command

Override with `--output json` or `--output table`.

```bash
# Table output (default in terminal)
tpv workflow list

# JSON for scripting
tpv workflow list --output json

# Or just pipe it
tpv workflow list | jq '.[] | .workflow_id'
```
