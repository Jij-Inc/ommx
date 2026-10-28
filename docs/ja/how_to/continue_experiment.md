# Experimentを継続・復帰する

編集中のExperimentにはRunを追加できます。中断した編集はcheckpoint（復帰用の保存地点）から復帰し、確定済みの記録を変えずに続きを作るときはforkします。どちらもソルバー内部の探索状態を再開する操作ではありません。

[SDKのインストール](../getting_started/install.md)に従って実行環境を用意してください。以下の小さな例にはAdapterも外部データも不要です。名前はこの例専用のものです。自分のデータでは別の名前に置き換えてください。

## 閉じたRunのcheckpointから復帰する

最初に一つのRunを閉じます。ここではExperiment全体はまだcommitしていません。標準の自動保存設定では、この時点でdraft checkpointが作られます。

```python
from ommx.experiment import Experiment

experiment = Experiment("example.com/book/checkpoint:demo")
with experiment.run() as run:
    run.log_parameter("trial", "first")
    run.log_json("result", {"value": 3})
```

ここでPythonプロセスやNotebook kernelを終了しても、次のコードから復帰できます。同じLocal Registry（OMMXのローカル保存領域）を使ってください。`OMMX_LOCAL_REGISTRY_ROOT`を指定していた場合は、新しいプロセスにも同じ値を設定します。

```python
from ommx.experiment import Experiment, list_experiment_checkpoints

name = "example.com/book/checkpoint:demo"
checkpoints = list_experiment_checkpoints(name)
assert checkpoints
print(checkpoints[0].status)

restored = Experiment.restore_from_checkpoint(name)
assert restored.runs[0].get_json("result") == {"value": 3}
with restored.run() as run:
    run.log_parameter("trial", "second")
    run.log_json("result", {"value": 2})
restored.commit()
assert len(restored.runs) == 2
```

復帰APIには、生成された内部checkpoint名ではなく、最初に指定したExperiment名を渡します。`commit()`すると、その名前で確定済み記録を読めるようになり、checkpointは削除されます。

復帰対象は最後に保存されたcheckpointまでです。開いたRunの途中状態は含まれません。自動保存を間引いた場合は、それ以降に閉じたRunも含まれないことがあります。`failed`や`interrupted`のcheckpointも同じAPIで復帰できますが、失敗した呼び出しを自動的に再実行するものではありません。

## 確定済みの記録からforkする

確定済みExperimentは読み出し専用です。既存の結果を残したまま試行を追加するときは、別の名前でforkします。以下は前節のデータに依存しない例です。

```python
from ommx.experiment import Experiment

with Experiment("example.com/book/fork:parent") as parent:
    with parent.run() as run:
        run.log_parameter("trial", "baseline")

with parent.fork("example.com/book/fork:child") as child:
    with child.run() as run:
        run.log_parameter("trial", "additional")

assert len(parent.runs) == 1
assert len(child.runs) == 2
```

子は親の添付データ、Run、Solve、Samplingを引き継ぎ、追加したRunとともに保存されます。親の内容は変わりません。変更していない添付データはLocal Registryで共有されます。

親を後から開くなら`Experiment.load("example.com/book/fork:parent")`、ファイルで受け取るなら`Experiment.import_archive(path)`を使い、その戻り値からforkできます。新しい名前はローカルでの識別にも使えるので、ここでネットワークへpushする必要はありません。

## 自動保存の頻度を変えたい場合

短いRunが多い場合は、編集中のExperimentに自動保存方針を設定できます。

```python
from ommx.experiment import AutosavePolicy, Experiment

experiment = Experiment("example.com/book/autosave:demo")
experiment.set_autosave_policy(AutosavePolicy.every_n_runs(25))
```

この例では、以降25個のRunが閉じられるごとにcheckpointします。復帰可能な地点が減ることと引き換えなので、単に最初の使い方として設定する必要はありません。セッション限りの設定であり、復帰後に同じ方針が必要なら改めて設定します。時間間隔・無効化・例外終了時の厳密な条件は{py:class}`~ommx.experiment.AutosavePolicy`を参照してください。

記録状態の意味は[ExperimentのGuide](../user_guide/experiment.md)、検索やcleanupは[CLIのGuide](../how_to/manage_local_registry.md)で説明します。
