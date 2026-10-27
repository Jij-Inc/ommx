# 問題と評価結果をファイルで受け渡す

**共有したいデータをArtifactに追加し、`commit()`してから`.ommx`ファイルに保存します。** 受け取る側は、そのファイルだけから問題・評価結果・分析メモを取り出せます。

ここでは二つの項目から一つまで選べる問題と、手元の候補の評価結果を渡します。候補を探すソルバーは使いません。`Instance`が問題、`State`が変数への値の割り当て、`Solution`がその候補を問題で評価した結果です。

## 準備

[インストール手順](../getting_started/install.md)の1・2で、Python SDKの`3.0.0b7`を用意してください。Adapter、Experiment、レジストリへのログインは不要です。

空の作業フォルダで、以下を二つのPythonファイルとして保存します。二つのスクリプトは**別々に実行**してください。送信側と受信側で保存場所を分け、受信側が送信側のメモリや保存済みデータに頼っていないことを確かめます。

## 送信側：三つのデータを一つのファイルに保存する

次を`send_artifact.py`として保存します。

```python
import os
from pathlib import Path

# この例専用の保存場所。初回に存在しないフォルダを指定する。
registry = Path("artifact-sender-registry").resolve()
registry.mkdir()
os.environ["OMMX_LOCAL_REGISTRY_ROOT"] = str(registry)

from ommx import Instance, State
from ommx.artifact import ArtifactDraft

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = 3 * x + 2 * y
instance.add_constraint(x + y <= 1)

solution = instance.evaluate(State({x.id: 1, y.id: 0}))
assert solution.feasible
assert solution.objective == 3

draft = ArtifactDraft.new_anonymous()
instance_layer = draft.add_instance(instance)
solution_layer = draft.add_solution(solution)
_ = draft.add_json(
    {
        "instance_digest": instance_layer.digest,
        "solution_digest": solution_layer.digest,
        "selected": ["x"],
        "note": "手元の候補を評価した結果。最適性の証明ではない。",
    },
    title="analysis",
)
artifact = draft.commit()
artifact.save("candidate.ommx")
print("candidate.ommxに保存しました")
```

SDKを導入した環境で実行します。

```bash
python send_artifact.py
```

`candidate.ommx`が共有用のファイルです。このファイルだけを相手に渡せます。`artifact-sender-registry`は送信側の**Local Registry**、つまりSDKのローカル保存場所であり、渡す必要はありません。

`add_instance()`などは、データをArtifactの一部分である**レイヤー**として追加します。`commit()`はその一覧を確定してLocal Registryに保存し、`save()`は共有用のファイルを書き出します。この二つは別の操作です。

## 受信側：新しいプロセスでファイルだけを読む

`candidate.ommx`と同じ場所に、次を`receive_artifact.py`として保存します。送信側の変数名や返り値は使いません。

```python
import os
from pathlib import Path

# 空のLocal Registryから始める。送信側とは別の場所を指定する。
registry = Path("artifact-receiver-registry").resolve()
registry.mkdir()
os.environ["OMMX_LOCAL_REGISTRY_ROOT"] = str(registry)

from ommx.artifact import Artifact

received = Artifact.import_archive("candidate.ommx")
instance = received.instance
solution = received.solution

analysis_layer = next(
    layer
    for layer in received.layers
    if layer.annotations.get("org.ommx.user.title") == "analysis"
)
analysis = received.get_json(analysis_layer)

checked = instance.evaluate(solution.state)
assert checked.feasible == solution.feasible
assert checked.objective == solution.objective
print(solution.feasible, solution.objective)
print(analysis["selected"])
```

新しいPythonプロセスとして実行します。

```bash
python receive_artifact.py
```

結果は`True 3.0`と`['x']`です。問題、候補の評価結果、JSONの分析メモを、ファイルから取得できました。`import_archive()`は内容を受信側のLocal Registryへ永続的に取り込みます。ファイルを開いたままその場で読むだけの操作ではありません。

この例は各種類のデータを一つずつ保存しているため、`.instance`と`.solution`で最初の問題・評価結果を取り出しています。JSONは保存時の`title="analysis"`を手がかりに選びました。メモの`instance_digest`と`solution_digest`には、メモが参照するレイヤーの内容識別子を明示的に記録しています。複数の問題や結果を含めるときは、{meth}`~ommx.artifact.Artifact.get_layer_descriptor`と型別のgetterで対象を選べます。

同じフォルダでスクリプトをやり直す場合は、既存データを消さず、作業フォルダまたは例中の保存先を新しい名前にしてください。`mkdir()`が失敗する場合は、前回の保存場所がすでに存在しています。

## ここで終えてよいこと・次にできること

受け渡したのは、入力した候補の評価結果です。保存・読み込みや候補の実行可能性だけで最適性が保証されるわけではありません。また、ソースコードや実行環境など、追加しなかった情報はこのファイルには入りません。

- 保存場所と名前の意味を知りたい場合：[Artifactの保存と識別](../user_guide/artifact.md)
- ファイルではなくレジストリで配布したい場合：[レジストリでArtifactを共有する](../how_to/share_artifact_registry.md)
- 複数の試行の条件・結果を整理したい場合：[Experiment Tutorial](experiment_management)。このファイル受け渡しにExperimentは必要ありません。

データ形式を別の言語やツールで扱う場合は、Pythonの手順ではなく[公開b7に対応するArtifact仕様](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md)を参照してください。
