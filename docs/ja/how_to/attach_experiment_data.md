# Experimentにデータを添付する

入力JSON、モデル、評価結果、ログや画像を、Experiment全体または特定のRun（一回の試行）に添付できます。まず標準の型付きメソッドかファイル添付を使います。基本的な記録と保存の流れは[二つの試行を記録するTutorial](../tutorial/experiment_management)を参照してください。

[SDKのインストール](../getting_started/install.md)に従って実行環境を用意してください。この例にはAdapterやソルバーは不要です。

(experiment-management-attachable-data-formats)=
## 標準のデータとファイルを保存する

次の例は、小さな入力ファイルを作り、共有の問題・JSON・ファイルと、Runの評価結果を保存します。`attachment-example/notes.txt`はこの例で作成・更新するファイルです。

```python
from pathlib import Path

from ommx import Instance
from ommx.experiment import Experiment

data_dir = Path("attachment-example")
data_dir.mkdir(exist_ok=True)
input_file = data_dir / "notes.txt"
input_file.write_text("candidate evaluated locally\n", encoding="utf-8")

instance = Instance.minimize()
x = instance.new_binary("x")
instance.objective = x

with Experiment() as experiment:
    experiment.log_json("source-data", {"description": "one binary variable"})
    experiment.log_instance("problem", instance)
    experiment.log_file("notes", input_file, media_type="text/plain")
    with experiment.run() as run:
        run.log_solution("evaluation", instance.evaluate({x.id: 0}))

experiment.save(data_dir / "attachments.ommx")
```

`log_file`は、その時点のファイル内容を取り込みます。元のファイルへのリンクではありません。後から元ファイルを編集しても、保存済み添付データは変わりません。`save`は同名ファイルを上書きしないので、再実行時は新しい保存先を選び、後の読込パスも合わせてください。

## 名前を確認して取り出す

型が分かっているときは型別の取得メソッドを使います。間違った種類のデータを指定した場合、暗黙に別の型へ変換せずエラーになります。

```python
from ommx.experiment import Experiment

loaded = Experiment.import_archive("attachment-example/attachments.ommx")
print(sorted(loaded.attachment_names))
source_data = loaded.get_json("source-data")
problem = loaded.get_instance("problem")
evaluation = loaded.runs[0].get_solution("evaluation")
assert source_data["description"] == "one binary variable"
assert problem.evaluate(evaluation.state).objective == evaluation.objective
assert loaded.get_blob("notes") == b"candidate evaluated locally\n"
```

`get_attachment(name)`はMedia Type（保存データの種類）を見て標準型を読み分けます。`get_blob(name)`はbytesが必要なときに使います。元のファイルとして書き出す場合は、保存先を指定します。

```python
from pathlib import Path

destination = Path("attachment-example/restored-notes.txt")
loaded.write_attachment("notes", destination, overwrite=True)
assert destination.read_text(encoding="utf-8") == "candidate evaluated locally\n"
```

ここでは例を再実行できるよう`overwrite=True`で指定先ファイルを更新します。既存ファイルを置き換えたくない場合はこの引数を省略し、新しいパスを指定してください。

## どこに置き、何を明示するか

全Runで共通の入力はExperiment、試行ごとに異なるデータはRunに置くと、後で対応を辿りやすくなります。添付名はそのスコープ内で取り出すための名前です。Runごとに同じ名前を使っても、別のRunの添付データです。

比較表の列にしたいスカラー値は`run.log_parameter`に記録します。添付JSON内の数値やAdapterに渡したオプションが、自動的にRun parameterへ展開されるわけではありません。

大きなJSONやファイルなどは`compression="zstd"`を指定できます。取得メソッドは展開された内容を返すので、読み手の操作は変わりません。対応するメソッドと厳密な制限は{py:class}`~ommx.experiment.Experiment`と{py:class}`~ommx.experiment.Run`を参照してください。

## 独自型を保存する場合だけCodecを使う

標準型でもファイルでも扱えないPythonオブジェクトには、`media_type`、`encode(value) -> bytes`、`decode(bytes) -> value`を持つCodecを`log_with_codec` / `get_with_codec`に渡せます。具体的なCodecは、原則としてその型を所有するパッケージ側が提供します。読み手にも対応するパッケージとCodecが必要です。

OMMXは外部型の意味やバージョン互換性を自動的に決めません。外部パッケージの公開Codecやシリアライズ仕様を確認してください。Protocolの呼び出し方とMedia Typeの検証は{py:meth}`~ommx.experiment.Run.log_with_codec`と{py:meth}`~ommx.experiment.SealedRun.get_with_codec`を参照してください。

記録の確定・再開は[ExperimentのGuide](../user_guide/experiment.md)、階層を使わないデータ交換は[ArtifactのTutorial](../tutorial/share_in_ommx_artifact.md)に分けています。
