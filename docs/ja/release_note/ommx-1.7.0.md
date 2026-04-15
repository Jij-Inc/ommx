---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

```{warning}
このドキュメントはOMMX Python SDK 1.7.0のリリース時のものであり、Python SDK 2.0.0以降では動作しません。
```

+++

# OMMX Python SDK 1.7.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.7.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.7.0)

個々の変更についてはGitHub Releaseを参照してください。

Summary
--------
- [英語のJupyter Book](https://jij-inc.github.io/ommx/en/introduction.html)
- QPLIBフォーマットのパーサー
- `ommx.v1.SampleSet` と `ommx.v1.ParametricInstance` にいくつかAPIが追加されました。またOMMX Artifactとの連携機能が追加されました。
  - `ommx.v1.SampleSet` については[新しい解説ページを参照してください](https://jij-inc.github.io/ommx/ja/ommx_message/sample_set.html)
  - OMMX ArtifactのサポートについてはAPI reference [ommx.artifact.Artifact](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact)及び[ommx.artifact.ArtifactBuilder](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder)を参照してください。
- `{Solution, SampleSet}.feasible` の挙動の変更

+++

QPLIBフォーマットのパーサー
---------------------------

MPS形式に続いて、QPLIBフォーマットのパーサーに対応しました

```{code-cell} ipython3
import tempfile

# Example problem from QPLIB
#
# Furini, Fabio, et al. "QPLIB: a library of quadratic programming instances." Mathematical Programming Computation 11 (2019): 237-265 pages 42 & 43
# https://link.springer.com/article/10.1007/s12532-018-0147-4
contents = """
! ---------------
! example problem
! ---------------
MIPBAND # problem name
QML # problem is a mixed-integer quadratic program
Minimize # minimize the objective function
3 # variables
2 # general linear constraints
5 # nonzeros in lower triangle of Q^0
1 1 2.0 5 lines row & column index & value of nonzero in lower triangle Q^0
2 1 -1.0 |
2 2 2.0 |
3 2 -1.0 |
3 3 2.0 |
-0.2 default value for entries in b_0
1 # non default entries in b_0
2 -0.4 1 line of index & value of non-default values in b_0
0.0 value of q^0
4 # nonzeros in vectors b^i (i=1,...,m)
1 1 1.0 4 lines constraint, index & value of nonzero in b^i (i=1,...,m)
1 2 1.0 |
2 1 1.0 |
2 3 1.0 |
1.0E+20 infinity
1.0 default value for entries in c_l
0 # non default entries in c_l
1.0E+20 default value for entries in c_u
0 # non default entries in c_u
0.0 default value for entries in l
0 # non default entries in l
1.0 default value for entries in u
1 # non default entries in u
2 2.0 1 line of non-default indices and values in u
0 default variable type is continuous
1 # non default variable types
3 2 variable 3 is binary
1.0 default value for initial values for x
0 # non default entries in x
0.0 default value for initial values for y
0 # non default entries in y
0.0 default value for initial values for z
0 # non default entries in z
0 # non default names for variables
0 # non default names for constraints"#;
"""

# 名前付きの一時ファイルを作成
with tempfile.NamedTemporaryFile(delete=False, suffix='.qplib') as temp_file:
    temp_file.write(contents.encode())
    qplib_sample_path = temp_file.name


print(f"QPLIB sample file created at: {qplib_sample_path}")
```

```{code-cell} ipython3
from ommx import qplib

# QPLIBファイルを読み込む
instance = qplib.load_file(qplib_sample_path)

# 決定変数と制約条件を表示
display(instance.decision_variables)
display(instance.constraints)
```

`{Solution, SampleSet}.feasible` の挙動の変更
---------------------

- `ommx.v1.Solution` と `ommx.v1.SampleSet` にある `feasible` の挙動が変更されました
  - Python SDK 1.6.0で導入された `removed_constraints` の扱いが変更されています。1.6.0では `feasible` は `removed_constraints` を無視していましたが、1.7.0では `removed_constraints` を考慮するようになりました。
  - 合わせて明示的に `removed_constraints` を無視する `feasible_relaxed` と考慮する `feasible_unrelaxed` を導入しました。`feasible` は `feasible_unrelaxed` のエイリアスになっています。


挙動を理解するために次の簡単な最適化問題を考えましょう

$$
\begin{aligned}
    \max &\quad x_0 + x_1 + x_2 \\
    \text{s.t.} &\quad x_0 + x_1 \leq 1 \\
                &\quad x_1 + x_2 \leq 1 \\
    &\quad x_1, x_2, x_3 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Instance

x = [DecisionVariable.binary(i) for i in range(3)]

instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x),
    constraints=[
        (x[0] + x[1] <= 1).set_id(0),
        (x[1] + x[2] <= 1).set_id(1),
    ],
    sense=Instance.MAXIMIZE,
)
instance.constraints
```

さらに制約条件の片方 $x_0 + x_1 \leq 1$ を緩和します

```{code-cell} ipython3
instance.relax_constraint(constraint_id=0, reason="Manual relaxation")
display(instance.constraints)
display(instance.removed_constraints)
```

さてそうすると $x_0 = 1, x_1 = 1, x_2 = 0$ は元の問題には解ではありませんが、緩和された問題には解です。なので `feasible_relaxed`は `True` になりますが、`feasible_unrelaxed` は `False` になります。`feasible`は`feasible_unrelaxed`のエイリアスなので`False`になります。

```{code-cell} ipython3
solution = instance.evaluate({0: 1, 1: 1, 2: 0})
print(f"{solution.feasible=}")
print(f"{solution.feasible_relaxed=}")
print(f"{solution.feasible_unrelaxed=}")
```
