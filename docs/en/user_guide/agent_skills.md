# Distribute Agent Plugins and Skills with OMMX

The OMMX CLI distributes an agent plugin bundle and exposes its paths from the
OMMX installed in your project, using the same command structure as
[JijModeling](https://jij-inc-jijmodeling-tutorials-en.readthedocs-hosted.com/en/latest/advanced/agent_plugin_installation.html).

The current `ommx/SKILL.md` is a distribution placeholder. OMMX usage guidance
will be provided separately. The following examples verify installation and
discovery of the bundle.

For an agent that discovers `.agents/skills`, link the skill from your project root:

```bash
mkdir -p .agents/skills
ln -s "$(uv run ommx skill path)/ommx" .agents/skills/ommx
```

Start a new session and confirm that `ommx` is listed with this project's
`SKILL.md` as its source. These shell examples use a POSIX shell.

## Find the plugin, marketplace, or skill

```bash
uv run ommx plugin path
uv run ommx plugin marketplace path
uv run ommx skill path
```

Each command prints one absolute directory path to stdout:

| Command | Directory contents |
| --- | --- |
| `plugin path` | `plugin.json`, `.claude-plugin/plugin.json`, and `skills/` |
| `plugin marketplace path` | `.claude-plugin/marketplace.json` and `plugins/ommx/` |
| `skill path` | `ommx/SKILL.md` |

Like JijModeling, plugin and skill names are optional positional arguments:

```bash
uv run ommx plugin path ommx
uv run ommx skill path ommx
```

`plugin path ommx` returns the same plugin as `plugin path`. `skill path ommx`
returns the individual skill directory instead of its parent. The bundled name
is `ommx`.

The standalone Rust executable supports the same commands. The Python module
entry point also works: `uv run python -m ommx.cli plugin path`.

## Install the plugin for a project

The bundle contains an [Agent Plugins 1.0](https://agent-plugins.org/specification)
manifest and Claude Code metadata. Install it through Claude Code's local
marketplace with project scope:

```bash
claude plugin marketplace add --scope project "$(uv run ommx plugin marketplace path)"
claude plugin install --scope project ommx@ommx
```

For one Claude Code session, pass the plugin directory at startup instead:

```bash
claude --plugin-dir "$(uv run ommx plugin path)"
```

Cursor CLI can also load the plugin for one session:

```bash
agent --plugin-dir "$(uv run ommx plugin path)"
```

Confirm in a new session that the agent discovers the OMMX skill from the
configured plugin. In Claude Code, its name is `/ommx:ommx`. See the
[Claude Code marketplace documentation](https://code.claude.com/docs/en/plugin-marketplaces)
for the marketplace format and installation behavior.

## Install the individual skill

Use the `.agents/skills/ommx` link shown above, or copy the complete skill directory:

```bash
mkdir -p .agents/skills
cp -R "$(uv run ommx skill path ommx)" .agents/skills/ommx
```

Run this with no existing `.agents/skills/ommx` directory. Skill managers that
expect the parent directory can use `skill path`, for example:

```bash
gh skill install "$(uv run ommx skill path)" ommx --from-local --scope project
```

Choose the agent when prompted. Confirm the skill's project scope and source
path in a new session. Add `.agents/skills/ommx` to `.gitignore` for
an environment-specific link or copy. If a team shares a copy in Git, update it
alongside the project's OMMX dependency. Claude Code project settings can also
contain absolute local marketplace paths; keep those local paths out of shared
configuration.

## Versions, cache, and updates

The plugin's manifests share the Rust CLI version shown by `ommx --version`,
including when launched through Python. OMMX maintains Rust and Python SDK
versions independently.

OMMX embeds the whole bundle in the Rust executable and Python wheel. Path
commands materialize it in the OS's OMMX cache under a key derived from all
bundled files, including manifest versions. No source checkout, network access,
or Artifact Local Registry is needed. Set `OMMX_PLUGIN_CACHE_DIR` to choose a
writable cache location. Cached files are generated data; subsequent calls
restore missing or altered entries.

After upgrading OMMX, obtain the paths again. Recreate symbolic links if their
targets changed, or replace the complete copied skill directory. For a local
Claude Code marketplace, register the new path before updating the plugin:

```bash
claude plugin marketplace add --scope project "$(uv run ommx plugin marketplace path)"
claude plugin update --scope project ommx@ommx
```

For `--plugin-dir`, launch a new session with the new path. Confirm that the
agent reads the updated skill. If the cache is deleted, run the path command
again to recreate the bundle before using links or marketplace registrations.
