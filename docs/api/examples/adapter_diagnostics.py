"""Render saved SCIP diagnostics; optionally collect a new air05 run.

From the repository root:
    uv run --no-sync python docs/api/examples/adapter_diagnostics.py

Pass --record to download/cache MIPLIB air05 and solve it again. Recording uses
the installed SDK and solver defaults and is never part of the rendering test.
"""

import argparse
from dataclasses import asdict
from datetime import datetime, timezone
from importlib.metadata import version
import json
import hashlib
from pathlib import Path
import platform
import subprocess

import numpy as np
from matplotlib import pyplot as plt
from ommx_pyscipopt_adapter import SCIPDiagnosticsAnalyzer


HERE = Path(__file__).resolve().parent


def record(path: Path) -> None:
    """Collect actual diagnostics, retaining all reports and acquisition context."""
    from ommx import dataset
    from ommx.adapter import DiagnosticCollector
    from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
    from pyscipopt import Model

    instance = dataset.miplib2017("air05")
    collector = DiagnosticCollector()
    solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)
    analysis = SCIPDiagnosticsAnalyzer(collector.diagnostics)
    payload = {
        "metadata": {
            "problem": "air05",
            "source": "ghcr.io/jij-inc/ommx/v2.7/miplib2017:air05",
            "recorded_at": datetime.now(timezone.utc).isoformat(),
            "source_revision": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], text=True
            ).strip(),
            "python": platform.python_version(),
            "platform": platform.platform(),
            "machine": platform.machine(),
            "ommx": version("ommx"),
            "ommx-pyscipopt-adapter": version("ommx-pyscipopt-adapter"),
            "pyscipopt": version("pyscipopt"),
            "sense": repr(instance.sense),
            "settings": "OMMXPySCIPOptAdapter.solve with its default preparation and solver settings",
            "scip_infinity": Model().infinity(),
            "solution_objective": solution.objective,
            "solution_feasible": solution.feasible,
        },
        "diagnostics": [asdict(report) for report in collector.diagnostics],
    }
    assert analysis.termination_result is not None
    path.write_text(json.dumps(payload, indent=2, allow_nan=False) + "\n")


def plot(payload: dict):
    """Plot callback observations, masking non-finite values and SCIP infinity."""
    analysis = SCIPDiagnosticsAnalyzer(payload["diagnostics"])
    history = analysis.progress_history_df
    columns = ["primal_bound", "dual_bound", "gap"]
    values = history[columns]
    # SCIP can use a finite sentinel for infinity. Mask it as well as NaN/Inf;
    # keep missing observations as gaps rather than joining across them.
    visible = values.where(
        np.isfinite(values) & (values.abs() < payload["metadata"]["scip_infinity"])
    )
    fig, (bounds, gap) = plt.subplots(2, 1, sharex=True, figsize=(7, 6))
    visible[["primal_bound", "dual_bound"]].plot(ax=bounds)
    visible["gap"].plot(ax=gap, label="SCIP relative gap", legend=True)
    bounds.set(
        ylabel="Solver objective bound", title="SCIP on air05 (one recorded run)"
    )
    gap.set(xlabel="Solver elapsed time (s)", ylabel="Relative gap")
    for ax in (bounds, gap):
        ax.grid(alpha=0.25)
    fig.tight_layout()
    return fig


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", type=Path, default=HERE / "scip_air05.json")
    parser.add_argument(
        "--output",
        type=Path,
        default=HERE.parent / "assets" / "adapter_diagnostics_bounds.png",
    )
    parser.add_argument("--record", action="store_true")
    args = parser.parse_args()
    if args.record:
        record(args.data)
    payload = json.loads(args.data.read_text())
    fig = plot(payload)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(
        args.output,
        dpi=160,
        metadata={
            "SourceSHA256": hashlib.sha256(args.data.read_bytes()).hexdigest(),
            "PlotScriptSHA256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        },
    )
    plt.close(fig)


if __name__ == "__main__":
    main()
