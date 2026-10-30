"""Keep the published diagnostic figure reproducible without a network or solve."""

import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys

from matplotlib import pyplot as plt
import numpy as np
from PIL import Image

from ommx_pyscipopt_adapter import SCIPDiagnosticsAnalyzer


API = Path(__file__).resolve().parents[3] / "docs" / "api"
SCRIPT = API / "examples" / "adapter_diagnostics.py"
DATA = API / "examples" / "scip_air05.json"
IMAGE = API / "assets" / "adapter_diagnostics_bounds.png"


def test_recorded_diagnostics_plot(tmp_path):
    payload = json.loads(DATA.read_text())
    analysis = SCIPDiagnosticsAnalyzer(payload["diagnostics"])
    assert analysis.termination_result is not None
    assert analysis.termination_result["status"] == "optimal"
    assert (
        analysis.termination_result["objective_value"]
        == payload["metadata"]["solution_objective"]
    )
    history = analysis.progress_history_df
    assert history.index.is_monotonic_increasing
    assert history.iloc[-1]["event"] == "TERMINATION"

    namespace = runpy.run_path(str(SCRIPT))
    fig = namespace["plot"](payload)
    try:
        lines = [*fig.axes[0].lines, *fig.axes[1].lines]
        assert len(lines) == 3
        for line, column in zip(lines, ["primal_bound", "dual_bound", "gap"]):
            np.testing.assert_array_equal(line.get_xdata(), history.index)
            values = history[column].to_numpy()
            expected = np.array(
                [
                    value
                    if np.isfinite(value)
                    and abs(value) < payload["metadata"]["scip_infinity"]
                    else np.nan
                    for value in values
                ]
            )
            np.testing.assert_allclose(line.get_ydata(), expected, equal_nan=True)
        assert lines[2].get_ydata()[-1] == 0.0
    finally:
        plt.close(fig)

    # The normal command only reads the saved reports: no --record or solve.
    output = tmp_path / "diagnostics.png"
    subprocess.run(
        [sys.executable, str(SCRIPT), "--data", str(DATA), "--output", str(output)],
        check=True,
    )
    with Image.open(output) as image:
        image.verify()
    # Catch a changed data/script with an accidentally stale committed figure.
    with Image.open(IMAGE) as image:
        assert (
            image.info["SourceSHA256"] == hashlib.sha256(DATA.read_bytes()).hexdigest()
        )
        assert (
            image.info["PlotScriptSHA256"]
            == hashlib.sha256(SCRIPT.read_bytes()).hexdigest()
        )
