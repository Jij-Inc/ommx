# Experimentの記録とライフサイクル

Experimentは、複数の試行で使った条件とデータをまとめて残すための機能です。問題の記述・変換・評価やAdapterの実行には必須ではありません。まず記録して読み直したい場合は[二つの試行を記録するTutorial](../tutorial/experiment_management)から始めてください。

## 何を、どの単位で記録するか

| 単位 | 記録するもの | 読み出し口 |
|---|---|---|
| Experiment | 一連の試行で共有する入力や説明、Runの集合 | `attachment_names`、型別の取得メソッド、`runs` |
| Run | 一回の試行のスカラー条件、添付データ、必要ならSolve・Sampling | `run_parameters_df()`、Runの取得メソッド、`solves`、`samplings` |
| Solve | 一回のSolverAdapter呼び出しの入力Instance、Adapter名・オプション、得られたSolution | `run.solves`の各要素の`input`、`output` |
| Sampling | 一回のSamplerAdapter呼び出しの入力Instance、Adapter名・オプション、得られたSampleSet全体 | `run.samplings`の各要素の`input`、`output` |

一つのRunはSolveやSamplingを複数持てます。どちらもなく、手元の候補の評価結果や外部で計算した値だけを記録してもかまいません。Tutorialはこの最小形を使います。

`log_parameter`はRunごとに比較したい数値・文字列・真偽値などのスカラー条件に使います。`run_parameters_df()`の列になるのは、ここに明示的に記録した値です。JSON、Instance、Solution、画像やログファイルなどはAttachment（添付データ）として保存します。添付データはExperiment全体にも、個別のRunにも置けます。手順は[データを添付するHow-to](../how_to/attach_experiment_data.md)を参照してください。

## Adapterを呼ぶと何が残るか

`run.log_solve(Adapter, instance, **options)`はAdapterの`solve()`を実行してSolveを記録し、`run.log_sample(...)`は`sample()`を実行してSamplingを記録します。入力Instance、Adapter名、渡したキーワード引数、返された結果が対応付けられます。渡したオプションは各呼び出しの記録であり、Run parameterの列には自動的にはなりません。

入力として残るのは、**loggingメソッドに渡したInstance**です。Adapterのeasy APIが内部で別のコピーをPreparationする場合、その変換後Instanceが自動的に保存されるわけではありません。また、選んだPolicy、適用した重み、バックエンドModelへの直接設定、Python変数、パッケージやOSの情報を網羅的に収集するものでもありません。

後で必要な情報は、その時点で明示的に記録します。例えば、呼び出し前に自分でPreparationしたInstanceを渡すなら、それが記録される入力です。元の問題やPolicyの選択理由も必要なら別の添付データとして残します。元のInstance自体が必要でない処理に、記録のためだけのコピーを追加する必要はありません。

手動でバックエンドModelを操作する必要がある場合には{py:meth}`~ommx.experiment.Run.open_solve`があります。通常の記録では`log_solve`を使い、細かな操作・診断・失敗時の契約はAPI Referenceを参照してください。

## 終了状態と結果の意味は別

RunやSolve、Samplingの`status`は、その記録スコープが正常終了したか、失敗したか、中断されたかを表します。

- `finished`は実行可能性や最適性を保証しません。実行可能な候補を含まないSampleSetでも、呼び出しが正常に返ればSamplingは`finished`です。
- Run内で子の呼び出しの例外を処理した場合、Runは`finished`でも、失敗したSolveやSamplingを含みえます。
- 実行可能性と目的値はSolutionやSampleSetから読みます。終了理由、bound、gapなどを取得できるかはAdapter・バックエンドと診断設定によります。

結果の基本的な解釈は[Solution](./solution.md)と[SampleSet](./sample_set.md)、変換後の解釈は[変換後の問題とRemoved constraints](./removed_constraints.md)に分けています。Experimentは、それらに新しい数学的保証を加えるものではありません。

## 編集中、確定、再開

Experimentは、編集できるセッションとして作成され、`commit()`に成功すると読み出し専用の記録になります。`with Experiment(...) as experiment:`の正常終了はcommitに相当します。短いスクリプトにはこの書き方が便利です。NotebookではExperimentを複数セルにまたがって開き、閉じたRunの結果を見ながら次のRunを追加し、最後に明示的にcommitしてもかまいません。

Runは`with experiment.run() as run:`で使います。Runを閉じると、その試行が親Experimentの記録に加わります。開いたRunがある間はExperimentをcommitできません。

| 状況 | 次に使う操作 |
|---|---|
| 同じ編集中セッションに次の試行を足す | `experiment.run()` |
| 記録を確定して読み出す | `commit()`、その後`Experiment.load(name)` |
| 確定済み記録を変更せず、新しい試行を追加する | `fork()`で子セッションを作る |
| 中断した編集の保存地点から続きを行う | `restore_from_checkpoint(name)` |
| ファイルから確定済み記録を読む | `Experiment.import_archive(path)` |

forkは既存のRunと添付データを引き継ぎ、親を変更しません。復帰は保存されたcheckpointを編集可能な状態で開きます。どちらもバックエンド内部の探索状態を再開する機能ではありません。[継続・復帰のHow-to](../how_to/continue_experiment.md)に操作を分けています。

## 保存される地点

データをlogするたびに、そのデータはLocal Registry（OMMXのローカル保存領域）に書かれます。ただし、データ本体が保存されたことと、後で取り出せるExperimentの保存地点ができたことは同じではありません。

通常はRunを閉じるごとに、そこまでの閉じたRunを含むdraft checkpointが自動保存されます。これはbest-effortの復帰地点です。`AutosavePolicy`で間隔を変えたり無効化したりできますが、その場合は最後のcheckpoint以降の試行が復帰対象に含まれないことがあります。開いたままのRunの途中状態は復帰可能なRunとして保存されません。

`with Experiment(...)`が例外で終わると、成功用の名前を更新せず、`failed`または`interrupted`のcheckpointを保存します。例外は呼び出し側に伝わります。コンテキスト管理を使わないセッションの外側の例外まで、自動的に捕捉するわけではありません。

正常commitは確定したExperimentを名前で取り出せる状態にし、そのセッションのcheckpointを削除します。保存地点と自動保存設定の厳密な条件は{py:class}`~ommx.experiment.Experiment`と{py:class}`~ommx.experiment.AutosavePolicy`を参照してください。

## ファイルを読むことと、計算をやり直すこと

確定済みExperimentはOMMX Artifactとして保存されます。`save()`でファイルへ書き出し、読み手は`import_archive()`で取り込めます。ファイルに保存した入力や結果は、作成元のPythonオブジェクトなしで読み直せます。

一方、同じ計算を再実行できるかは、明示的に残した入力・コード・設定や利用可能な実行環境に依存します。ファイルの読出し確認だけで再実行まで保証したことにはなりません。Tutorialでは前者を確認します。

試行の階層を使わないデータ交換は[Artifactの保存と識別](./artifact.md)、検索や整理は[Local RegistryのHow-to](../how_to/manage_local_registry.md)を参照してください。Local Registryの内部cacheやGC実装は、Experimentの最初の利用で理解する必要はありません。

## Local Registryの管理

保存した記録の名前を削除・復元し、不要なデータ本体を回収する手順は、{ref}`CLIの管理手順 <experiment-cleanup-workflow>`を参照してください。名前を削除する操作とデータ本体を回収する操作は別なので、削除前に対象と保存先を確認してください。
