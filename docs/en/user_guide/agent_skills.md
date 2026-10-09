# Use OMMX with Coding Agents

OMMX bundles a skill for coding agents that work with its Python SDK, solver
adapters, Artifacts, and Experiments. Obtain it from the OMMX installed in your
project so the instructions follow the SDK you use.

From your project root, link the skill into the project's skill directory:

```bash
mkdir -p .agents/skills
ln -s "$(uv run ommx skill path --name ommx)" .agents/skills/ommx
```

Start a new session in an agent that discovers `.agents/skills` and confirm that
it lists `ommx` with this project's `SKILL.md` as the source. Add
`.agents/skills/ommx` to your project's `.gitignore` when the link is specific to
your environment. These shell examples use a POSIX shell.

## Find or copy the skill

```bash
uv run ommx skill path
uv run ommx skill path --name ommx
```

Both commands print one absolute directory path to stdout. The first returns a
skills parent directory containing `ommx/SKILL.md`; `--name ommx` returns the
`ommx` directory itself. Only the bundled name `ommx` is accepted.

To copy the skill instead of creating a symbolic link:

```bash
mkdir -p .agents/skills
cp -R "$(uv run ommx skill path --name ommx)" .agents/skills/ommx
```

Run this with no existing `.agents/skills/ommx` directory. A copy shared in Git
should be updated alongside the project's OMMX dependency. External skill
managers that expect a parent directory can consume `ommx skill path`, as in
the [JijModeling installation workflow](https://jij-inc-jijmodeling-tutorials-en.readthedocs-hosted.com/en/latest/advanced/agent_plugin_installation.html).

The same commands are available from the standalone Rust executable as
`ommx skill path` and `ommx skill path --name ommx`. The Python module entry
point also works: `uv run python -m ommx.cli skill path`.

## Cache and updates

The skill is embedded in the CLI, including the Python wheel. `skill path`
materializes it under the OS's OMMX cache directory, using a subdirectory keyed
by the skill's contents. It needs no network access or source checkout and does
not open the Artifact Local Registry. Set `OMMX_SKILL_CACHE_DIR` to choose a
different writable cache directory. Treat the cached files as generated data;
the next invocation restores a missing or altered `SKILL.md`.

After upgrading OMMX, run `skill path` again. If the returned path changed,
recreate your project's symbolic link; for a copy, replace the complete skill
directory. Then start a new agent session and confirm the skill's source path.
Deleting the cache also removes the target of any links; run the command again
to recreate it. Separate SDK installations with different skill contents use
different cache paths.
