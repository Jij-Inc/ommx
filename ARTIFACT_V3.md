# OMMX Experiment / Artifact v3 提案

OMMX v3 における Experiment / Run / Solve / Attachment / Trace / Lineage / GC の未実装領域をまとめる提案。

本ファイルは開発中の一時文書である。実装完了後は削除し、内容を通常の Sphinx documentation / API reference / module rustdoc に統合する。既に実装済みの Artifact manifest format、Local Registry、archive / remote transport の移行ログは本書では扱わない。必要な前提は `rust/ommx/doc/artifact_design.md` と `ommx::artifact::local_registry` の rustdoc を参照する。

本書の主眼は、MINTO が提供していた実験管理 UX を OMMX-owned な機能として再設計し、記録データ、実行 telemetry、Artifact version、lineage を一貫したモデルに落とすことである。

## 0. 用語集

本書では、データモデル上の状態 / 不変条件と、高レベルの操作を分けて扱う。操作名は「どの状態遷移を起こすか」によって定義し、状態名の代用にはしない。

### 0.1 データモデル上の用語

| 用語 | 意味 |
|---|---|
| Descriptor | OCI descriptor。digest / size / media type / annotations を主張するが、その bytes が OMMX Local Registry に存在することは保証しない。 |
| StoredDescriptor | 特定の Local Registry の BlobStore に、descriptor が指す bytes が存在することを保証する OMMX 側の型。保証するのは存在であり、その呼び出しが新規に書いたことではない。 |
| Unsealed | 複数 blob からなるデータ構造の一部または全部の component blob は保存済みだが、全体を指す root manifest blob はまだ保存されていない状態。データモデル上の mutable state。 |
| SealedArtifact | root manifest blob が BlobStore に保存され、全体を指す root descriptor が存在する状態を表す OMMX 側の型。inner descriptor は `StoredDescriptor` だが、component blob ではなく root manifest であることを型で区別する。 |
| Published | sealed root descriptor が Local Registry の IndexStore で ref に対応づけられた状態。publish は名前解決の更新であり、payload blob の保存ではない。 |

### 0.2 API lifecycle 上の用語

| 用語 | 意味 |
|---|---|
| Draft | ユーザーまたは SDK が変更中の mutable object。`ArtifactDraft` や `Experiment` は unsealed state を所有するが、`Draft` 自体は storage state ではない。 |
| LocalArtifact | sealed / published artifact を読むための immutable view。mutable session に戻す API は持たない。 |

### 0.3 操作に関する用語

| 操作 | レイヤ | 状態遷移 / 責務 |
|---|---|---|
| Store | Local Registry / BlobStore | bytes を content-addressed blob として保存し、検証後に `StoredDescriptor` を得る。 |
| Seal | データモデル | unsealed state から root manifest blob を作り、`SealedArtifact` を得る。ref は更新しない。 |
| Publish | Local Registry / IndexStore | `SealedArtifact` の root descriptor を ref に対応づけ、Published state にする。payload blob は保存しない。 |
| Commit | SDK lifecycle | Draft / Experiment を immutable Artifact として確定する高レベル操作。内部で必要な Store、Seal、通常は Publish を行う。 |
| Import | SDK / Registry boundary | Local Registry 外の source（OCI directory / archive / remote registry など）から identity を保って bytes と descriptors を取り込む高レベル操作。内部で source の読み取り、Store、必要に応じた Publish を行う。 |

`Staged` は data model の用語として使わない。以前 `Draft` / `Staged` が混在して表していた「blob は逐次保存されるが、全体としてはまだ root manifest を持たない」状態は `Unsealed` と呼ぶ。

## 1. 現行 UX

OMMX が提供する実験管理機構の中心 UX は、Experiment を実験全体の mutable session として扱い、正常終了時に immutable Artifact として commit することである。現在の Python API では、`Experiment()` は default Local Registry 上に anonymous Experiment を作り、名前を指定する場合は `Experiment(image_name)` に OCI image reference を渡す。`with Experiment(...) as exp:` は正常終了時に自動 commit し、例外終了時は commit しない。

```python
from ommx.experiment import Experiment
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

with Experiment("ghcr.io/jij-inc/ommx/scip_reblock115:latest") as exp:
    exp.log_json("dataset", "miplib2017")
    exp.log_json("source_problem", "reblock115")

    for formulation in formulations:
        with exp.run() as run:
            run.log_parameter("formulation", formulation.name)
            run.log_json("disabled_constraints", formulation.disabled_constraints)

            candidate = formulation.apply(instance)
            run.log_instance("candidate", candidate)

            solution = run.log_solve(
                OMMXPySCIPOptAdapter,
                candidate,
                time_limit=1.0,
            )

artifact = exp.artifact
run_parameters = exp.run_parameters_df()
runs = {run.run_id: run for run in exp.runs}
```

保存済み Experiment は read-only view として読み戻す。現在の実装では、既存 Artifact に Run を追加する `fork(...)` / lineage API はまだない。

```python
from ommx.experiment import Experiment

exp = Experiment.load("ghcr.io/jij-inc/ommx/scip_reblock115:latest")

artifact = exp.artifact
experiment_attachments = exp.experiment_attachments
run_parameters = exp.run_parameters_df()
runs = exp.runs
solves = runs[0].solves
```

この実装済み UX で保証したいこと:

- experiment 全体のデータと run 固有のデータを明確に分離できる。
- dataset / source problem / baseline configuration は experiment space に JSON Attachment として記録できる。
- run ごとに変わる formulation / strategy などの parameter を `run_parameters_df()` の列として記録し、後から比較できる。
- Instance、Solution、SampleSet、JSON、user-defined media type は Experiment / Run のどちらにも Attachment として記録できる。
- parameter は Attachment ではなく、Experiment が持つ Run 一覧に付随する表形式データとして扱う。
- solver adapter に実際に渡した kwargs は Run parameter ではなく、JSON 文字列化した Solve parameter として記録する。
- `log_*` は logger API ではなく、Experiment state への記録 API である。
- `Run` の context manager は Run lifecycle を閉じ、Run local state を Experiment state に反映する。
- `Experiment` の context manager は mutable session の lifecycle を閉じ、正常終了時に immutable Artifact version を自動 commit する。
- Artifact は digest primary、tag は mutable alias として共有に使う。
- Artifact を Local Registry から reload して、Experiment attachments、Run list、Solve list、Run parameter table を再構成できる。

Fork / lineage、trace diagnostics、failed recovery / autosave metadata は未実装であり、後続節の設計事項として扱う。

## 2. 参照する既存 UX

MINTO 互換 API は維持しない。ただし、以下のユーザー体験は OMMX 側に移す。

### 2.1 MINTO から維持する体験

MINTO の中心 UX は、実験全体に属するデータと run ごとに属するデータを分ける 2 つの保存空間モデルである。OMMX ではこの区分を Experiment space / Run space として維持する。

| 保存空間 | 目的 | 例 |
|---|---|---|
| Experiment space | 全 run で共有される context | dataset name, source problem, baseline config |
| Run space | 各 run の定式化、条件、実行環境、結果 | run parameters, run environment, candidate instance, disabled constraints, solution, sample set |

MINTO の archive / registry sharing UX も維持する。ただし OMMX では共有の正本は Artifact である。`save_as_ommx_archive()` / `load_from_ommx_archive()` のような名前は compatibility layer として置いてもよいが、中心概念は `commit()`、`Experiment.load(...)`、`Artifact.load(...)` に寄せる。

MINTO の `Experiment` context manager は `auto_saving` が有効なら保存 directory へ flush していたが、archive commit までは行っていなかった。OMMX では BlobStore-backed autosave を常に持つため、`with Experiment(...)` の意味を「保存 directory への flush」ではなく「Experiment session の正常終了と Artifact commit」に寄せる。

### 2.2 MLflow / W&B との差分

MLflow や W&B は主に Run lifecycle を context manager で閉じる。OMMX でも `with exp.run()` は Run を close / finalize する。

一方、OMMX の実験管理では Experiment が単なる Run の namespace ではなく、共有入力、複数の定式化、比較 table、Artifact sharing をまとめる単位になる。数理最適化では、同じ source problem から定式化を変える、一部の制約を無効化する、relaxation を変える、solver parameters を変える、といった複数 run を比較する作業が多い。このため OMMX では `with Experiment(...)` も lifecycle を持ち、正常終了時に experiment bundle を Artifact として seal する。

## 3. Lifecycle

### 3.1 Build / Seal / View

Experiment / Artifact の変更可能性は 3 相に分ける。

| 相 | 性質 | API |
|---|---|---|
| Build | mutable。payload blob は `log_*` 時点で BlobStore に保存する | `Experiment`, `Run` |
| Seal | immutable Artifact version を作る | `commit()` / context manager exit |
| View | read-only | `Artifact`, loaded `Experiment`, `run_parameters_df()` / descriptor views |

永続化済み Artifact を更新する API は作らない。既存 Artifact から新しい version を作る場合は、parent を lineage として記録する方針だが、fork / lineage API は未実装である。

### 3.2 Experiment context manager

`with Experiment(...)` は mutable session の lifetime を表す。

- 正常終了時は自動 commit する。
- 例外終了時は成功 commit を行わない。
- failed recovery manifest / autosave metadata は未実装である。
- block 内で明示 `commit()` 済みの場合、`__exit__` の commit は no-op にする。
- commit 後の `log_*` / `run()` は禁止する。
- `exp.artifact` は commit 後に available とし、commit 前アクセスは error にする。
- context manager を使わない場合は、明示 `exp.commit()` を呼ぶ。

`1 Experiment commit = 1 Artifact manifest` をデフォルトとする。`Run` 終了ごとに manifest を切る挙動は初期設計では提供しない。必要になった場合も opt-in とし、通常の比較 UX を複雑にしない。

Run は Artifact sub manifest ではなく、Experiment config 内の `run_id` で束ねられる logical entity とする。Run の保存実体は、Run parameter table の row、Run-scoped Attachment への `LayerRef`、Run 内の Solve list である。trace layer 内の `ommx.run` span は未実装の diagnostics / trace 設計事項である。

### 3.3 Run context manager

`with exp.run() as run:` は Run lifecycle を表す。

- Run 開始時に `run_id` を採番する。
- `run.log_parameter(...)` は Run table の cell を更新する。
- `run.log_instance(...)` / `run.log_solution(...)` / `run.log_sample(...)` は Run-scoped Attachment を追加する。`run.log_solve(...)` は Run 内の Solve を追加する。
- 正常終了時は Run を close し、Run local state を Experiment state に反映する。
- 例外終了時の status / diagnostics / recovery metadata の扱いは後続設計で決める。
- 例外は握りつぶさない。

Run close は Artifact commit ではない。Run close は Experiment state を更新し、autosave metadata に反映される。

### 3.4 常時 BlobStore-backed Autosave

`autosave` は user-facing な有効 / 無効 option にはしない。`Experiment` は常に BlobStore-backed autosave を持ち、`autosave=False` はサポートしない。

常時 autosave は「各 `log_*` のたびに最終 Artifact を commit する」ことではない。

目標:

- 大きな payload bytes を commit 前に BlobStore へ退避し、process crash だけで失われないようにする。
- autosave metadata が残っている場合は、記録済み entries を best-effort に復元できる。
- final commit までは public tag / digest を進めない。
- 大きな payload は `log_*` 時点で Local Registry の BlobStore に CAS blob として逐次保存する。
- final commit までは requested public success manifest / ref を publish しない。例外終了時の recovery manifest は reserved ref として別扱いにする。
- commit 時に作る manifest は、復元に必要な descriptor を完全に列挙する。

`log_*` 時点で BlobStore に書かれた Instance / solver log / diagnostics payload は、成功 commit 前にはどの public success manifest からも到達できない。process kill などで recovery manifest publish まで進めなかった場合、BlobStore には blob だけが残り、対応する manifest / ref は存在しない。この状態は corruption ではなく orphan blob として扱い、GC の対象にする。

orphan blob だけでは、どの Experiment / Run / Solve / Attachment に属していたかを復元できない。Experiment session として復元するには、`run_id`、`solve_id`、Attachment name、media type、blob digest、Run parameter table などを結ぶ autosave metadata が必要である。この metadata が残っている範囲では recovery command が session を再構成できる。metadata がない blob は単なる orphan blob として扱い、grace period 後に GC 対象にする。

例外終了を検知できた場合は、成功 Artifact と同じ tag には publish せず、`status=failed` / `recovery=true` 相当の annotation を持つ recovery manifest を作る。この manifest はその時点で分かっている Experiment config draft、Run parameter table、autosave metadata への link を含み、reserved ref で Local Registry に保持する。reserved ref は既存 anonymous artifact と同じ形式に合わせ、例えば `<registry-id8>.ommx.local/crashed:<local-timestamp>-<nonce>` とする。これにより、通常の共有 ref は進めずに、失敗した Experiment の途中成果だけを recovery command から辿れる。

recovery manifest の publish 自体に失敗した場合は、BlobStore には orphan blob だけが残り得る。この場合は自動的な Experiment 復元はできず、GC の grace period 内に low-level inspection する程度に留まる。

autosave の内部 metadata format は user-facing compatibility surface にしない。directory layout compatibility より、復元可能性と final Artifact semantics を優先する。commit されずに残った autosave metadata や、どの manifest からも到達しない blob は GC の対象になる。

### 3.5 Fork session

保存済み Experiment に Run を追加する操作は、既存 Artifact の再オープンではなく、loaded Experiment view から fork した新しい mutable session として扱う。

forked session では parent に含まれる既存 Attachment、Run、Solve、Run parameter table は読み取り可能な初期 state として見える。ただし parent Experiment / Artifact 自体は immutable であり、変更は child Artifact の manifest にだけ反映される。

新しい Run は既存 `run_id` と衝突しない id を割り当てる。正常終了時の自動 commit では parent を `subject` に持つ child manifest を作る。例外終了時は child Artifact を commit しない。autosave metadata による recovery は forked session を復元するためのものであり、parent Artifact を変更するものではない。

## 4. Experiment state model

Experiment state は以下からなる。

| 要素 | 内容 |
|---|---|
| Attachment list | Experiment / Run に添付された payload の順序付き list |
| Run parameter table | `run_id` と parameter name を key にした scalar table |
| Solve list | Run 内で発生した solver 呼び出し。input / output / solve parameter を持つ |

汎用保存抽象は domain object ではなく「payload を保存できる添付物」に近いため `Attachment` と呼ぶ。`Run` と `Solve` は domain object として扱い、Attachment はそれらに添付される payload descriptor への参照である。

```text
Experiment
  attachments        # Experiment scoped attachments
  runs
    parameters       # Run scoped scalar parameters
    attachments      # Run scoped opaque / auxiliary payloads
    solves
      input          # Instance
      output         # Solution
      parameters     # SolverAdapter に実際に渡された solve scoped parameters
```

`Run` は分割戦略、定式化戦略、アルゴリズム設定などの試行条件を表す。列生成法のように 1 つの run 内で master problem / pricing problem / subproblem を複数回 solve するケースがあるため、`Run` と `Solve` は 1:1 ではない。`Solve` は Run 内で発生した solver 呼び出しを表し、adapter 名、solver kwargs、backend status、elapsed time などは Run parameter ではなく Solve scoped metadata / parameters として扱う。

`Run` / `Solve` は Artifact sub manifest ではない。Artifact boundary は引き続き Experiment のみであり、Run と Solve は Experiment config 内の logical entity とする。

### 4.0 LayerRef と Descriptor の扱い

OCI では Descriptor は blob そのものではなく、blob を指す JSON metadata である。Descriptor 単体を保存する専用 blob はない。OMMX Artifact では payload blob を指す Descriptor は root OCI Image Manifest の `layers[]` に列挙される。

Experiment config が payload を参照する場合、同じ Descriptor を config 内に inline で複製すると、manifest `layers[]` 側の Descriptor と config 側の Descriptor が不一致になり得る。これを避けるため、目標モデルでは config 内に payload Descriptor を inline しない。代わりに zero-based `LayerRef` を使って、この config を所有する OCI Image Manifest の `layers[]` を参照する。

```text
LayerRef = u32  # index into the owning OCI Image Manifest's layers[]

ImageManifest.layers
  [0] Descriptor for experiment attachment
  [1] Descriptor for run 0 solve 0 input
  [2] Descriptor for run 0 solve 0 output

ExperimentConfig
  attachments: [LayerRef(0)]
  runs:
    - run_id: 0
      solves:
        - solve_id: 0
          input: LayerRef(1)
          output: LayerRef(2)
```

`LayerRef` は単独では意味を持たず、その config blob を `config` として参照している exact OCI Image Manifest と一緒にだけ解釈できる。`layers[]` の順序は manifest JSON の内容であり、manifest digest の一部である。順序を書き換えた場合は別 manifest / 別 Artifact になるため、OMMX Artifact としては layer order を semantic な descriptor table として扱ってよい。

Validation rules:

- `LayerRef.index < manifest.layers.len()`。
- `Solve.input` が参照する layer は OMMX Instance media type を持つ。
- `Solve.output` が参照する layer は OMMX Solution media type を持つ。
- Experiment / Run attachments は任意 media type を許す。
- `LayerRef` の重複参照は許してよい。同一 payload を複数 logical entity が参照することは合法である。
- loader は config と、それを所有する manifest の `layers[]` を合わせて解釈し、`layers[]` scan だけで Experiment / Run / Solve を推測しない。

Descriptor annotations は source of truth ではない。Config の構造と `LayerRef` が所属関係の正である。Descriptor annotation を持たせる場合も、generic inspector / compatibility / debug 用の補助情報に限定し、`run_id` / `solve_id` のような階層情報を重複保持しない。

### 4.1 Experiment space / Run space

OMMX では `log_global_*` という命名は採用しない。Experiment object に対する `exp.log_*` は experiment space に、Run object に対する `run.log_*` は run space に記録する。保存先は receiver で決まり、暗黙の context では決めない。

ただし `parameter` は Attachment ではなく Run parameter table の列データであり、Experiment には `exp.log_parameter` を持たせない。Experiment 全体に属する dataset、source problem、baseline、analysis context は JSON Attachment または media type を明示した Attachment として扱う。

```python
exp.log_json("dataset", "miplib2017")          # experiment space

with exp.run() as run:
    run.log_instance("candidate", instance)    # run space
    run.log_parameter("seed", 0)               # run table parameter
    run.log_solution("result", solution)       # run space
```

### 4.2 Attachment

Attachment は API / loader から見える名前付き payload であり、実装上の blob 所有単位を意味しない。domain object ではなく添付 payload であるため `Attachment` と呼ぶ。各 Attachment は概念的に owner（Experiment または Run）、name、content、media type、annotations を持つ。Attachment の分類軸は独立した enum ではなく OCI descriptor の `mediaType` に統一する。

| Field | 内容 |
|---|---|
| `owner` | `experiment` または `run` |
| `run_id` | run space の場合のみ必須 |
| `name` | space + media type 内の user-facing key |
| `content` | scalar value、serialized bytes、または blob descriptor への参照 |
| `media_type` | Artifact layer media type。OMMX core の既知型でも user / external package の独自型でもよい |
| `annotations` | Artifact descriptor annotations に投影される metadata |

Attachment media type は owner によって制限しない。例えば全 run で共有する source Instance は Experiment attachment に、実際に各 run で解いた candidate Instance は Run attachment または Solve input に置く。

代表的な media type:

| Media type | Payload | 備考 |
|---|---|---|
| `application/json` | JSON | dataset name / source problem id / config / small structured context |
| `application/org.ommx.v1.instance` | `ommx.v1.Instance` bytes | public API は `ommx.v1` |
| `application/org.ommx.v1.solution` | `ommx.v1.Solution` bytes | table summary は derived view として扱う |
| `application/org.ommx.v1.sample-set` | `ommx.v1.SampleSet` bytes | table summary は derived view として扱う |
| solver / adapter diagnostic media type | JSON または bytes | raw solver log、termination report、gap timeline など |
| user-defined media type | 任意の bytes | user / external package が所有する opaque payload |

user-defined media type の Attachment は、OMMX core が schema を知らない payload の escape hatch とする。caller は bytes と `media_type`、必要なら codec identifier や annotations を指定できる。OMMX core は unknown media type を decode せず、digest / size / media type / annotations を Artifact descriptor として保持する。

OMMX core は `jijmodeling` を import しない。domain-specific problem storage は external package が `media_type` と codec を登録して提供する。例えば `jijmodeling` の model payload は、`jijmodeling` package が media type / codec を所有し、OMMX には user-defined media type の Attachment として渡す。OMMX は descriptor を保持するだけで、parse / validation / round-trip guarantee はその media type owner の責務にする。

### 4.3 Run parameter table

Run parameter table は Attachment とは別に、`run_id` と parameter name を key にした scalar table として持つ。Parameter は最終的に pandas DataFrame / Apache Arrow の column として見せることを前提に、column ごとに型を固定する。cell value として受け付けるのは `bool`、`int64`、`float64`、`string` のみとし、`null`、array、object は受け付けない。欠損は `null` value ではなく、その `(run_id, parameter_name)` cell が存在しないことで表す。

```python
run.log_parameter("timelimit", 1.0)
run.log_parameter("seed", 0)
```

この 2 つは論理的には別 parameter cell である。ただし物理的には、run parameter aggregate JSON にまとめて保存してよい。実行中の `Run` は row-local な parameter map を持つが、column type の確定は commit 時の集計で行う。`int64` と `float64` が混在した column は `float64` に昇格し、それ以外の型混在は commit error にする。構造値や型混在を意図的に保存したい場合は、ユーザーが JSON を string 化して parameter に入れるか、Attachment として `log_json` / `log_attachment` する。

commit 時に materialize する Run parameter table JSON は column-oriented とする。

```json
{
  "columns": {
    "timelimit": {
      "type": "float64",
      "values": {
        "0": 1.0,
        "1": 10.0
      }
    },
    "solver": {
      "type": "string",
      "values": {
        "0": "scip",
        "1": "highs"
      }
    }
  }
}
```

`values` の key は `run_id` であり、存在しない key は missing cell を意味する。

物理化候補:

| 対象 | 候補 | 備考 |
|---|---|---|
| run parameter | Run ごとの parameter aggregate JSON | `run_parameters_df()` の入力になる |
| solve parameter | Solve ごとの string metadata map として Experiment config に含める | adapter 名、`json.dumps` した kwargs など。Run parameter には入れない |

この方針では、Run `parameter` は API / analysis 上は key 単位で扱えるが、Attachment ではなく Artifact layer の最小単位でもない。`Instance` や `Solution` のような大きな typed payload は Attachment / Solve input / Solve output として即 blob 化しやすいが、Run scalar parameter は commit 時に Run table payload として materialize する方が manifest / blob 数を抑えられる。Solve parameter は string metadata として Solve entry 内に保存し、kwargs は Python の `json.dumps` 結果を再解釈せず文字列のまま保持する。

### 4.4 Run metadata

Run status、elapsed time、実行環境属性を Experiment data model にどう保存するかは未決定とする。この PR では Run attributes という aggregate payload を導入しない。

### 4.5 Build phase と Seal phase

Build phase では Attachment / Solve input / Solve output は `log_*` 呼び出しごとに logical entry を追加する。Blob 本体は digest によって BlobStore で dedup されるが、Experiment config 内の Attachment list と Solve list は保存順と重複を保持する。`name` や `mediaType` は payload の属性であり、upsert key ではない。

Run parameter は Attachment ではなく Run table の cell なので、同じ `(run_id, parameter_name)` に対する parameter upsert を許容する。Solve parameter は Solve entry 内の metadata であり、Run table へ upsert しない。

Seal / commit phase では最終 Experiment state を復元できる aggregate payload と `LayerRef` に固定する。Committed config では Attachment list と Solve list をそのまま保存し、logical entry の dedup / normalization は行わない。

View phase の `Artifact` / loaded `Experiment` は immutable とし、追加や更新は `fork()` から別 Artifact を作る。

## 5. Adapter execution and diagnostics

### 5.1 Adapter 実行

OMMX では Adapter API が `SolverAdapter.solve(...)` / `SamplerAdapter.sample(...)` として標準化されているため、主要 UX は `run.log_solve(...)` / `run.log_sample(...)` にする。任意 callable を包む generic solver logging API は提供しない。

目標:

- `log_solve` は `SolverAdapter.solve(...)` を呼び、Run 内に Solve entry を追加する。
- `log_sample` は `SamplerAdapter.sample(...)` を呼び、Run 内に SampleSet-oriented Solve / Sample entry を追加する。詳細な型分けは後続設計で決める。
- adapter name / backend solver name / JSON-serializable kwargs は Run parameter ではなく Solve parameters として記録する。kwargs は `json.dumps` の結果を文字列として保存する。
- `log_solve` は caller が渡した original Instance を Solve input として保存し、Adapter が返した Solution を Solve output として保存する。
- 全 run で共有される巨大な Instance は Experiment attachment として明示的に保存し、Solve input から参照する API は後続設計で検討する。
- 実行時間は OTel span から得ることを基本とし、Artifact に保存する結果 payload の正本にはしない。
- adapter が返す diagnostics を Artifact-backed evidence として記録する。

```python
with exp.run() as run:
    solution = run.log_solve(
        OMMXPySCIPOptAdapter,
        instance,
        time_limit=1.0,
    )

with exp.run() as run:
    sample_set = run.log_sample(
        OMMXOpenJijSAAdapter,
        instance,
        num_reads=100,
        seed=0,
    )
```

### 5.2 Diagnostics

ソルバーの native log / report は、構造化された結果の正本ではない。しかし、モデルが解けない、timeout する、infeasible になる、または想定外の挙動をするときの解析には不可欠な診断 evidence である。OMMX はこの evidence を Artifact に保持できる必要がある。

どれだけの診断情報を保存すべきかはソルバー依存である。OMMX core が stdout / stderr を盲目的に capture し、すべての backend に対する policy を決めるべきではない。代わりに、各 OMMX Adapter が対象 backend solver の診断 policy を持つ。

Adapter は Experiment / Run を直接操作しない。Adapter の直接利用は従来通り `Solution` / `SampleSet` だけを返し、Experiment 管理の有無によって暗黙に挙動を変えない。

```python
solution = OMMXPySCIPOptAdapter.solve(instance)
```

Diagnostics は optional sink protocol として opt-in にする。`run.log_solve(...)` / `run.log_sample(...)` は Run-scoped `DiagnosticsSink` を作り、対応している Adapter に明示的に渡す。未対応 Adapter は単に diagnostics を返さないだけで、solve / sample 自体は従来通り動く。

```python
with exp.run() as run:
    solution = run.log_solve(
        "scip",
        OMMXPySCIPOptAdapter,
        instance,
        time_limit=1.0,
    )

# log_solve 内部の概念:
sink = run.diagnostics_sink(adapter="scip")
solution = OMMXPySCIPOptAdapter.solve(
    instance,
    time_limit=1.0,
    diagnostics=sink,  # Adapter が対応している場合だけ使われる
)
```

直接利用でも user が diagnostics を欲しい場合は、`DiagnosticCollector` を明示的に渡せるようにする。

```python
collector = DiagnosticCollector()
solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)
diagnostics = collector.entries
```

Adapter 側の責務:

- debug に有用な native log / report / summary を選ぶ。
- presolve log、node log、gap history、termination report、warning、backend status、backend version、sampler schedule など、ソルバー固有の evidence を capture する。
- truncate、compression、redaction、summary の policy を決める。
- optional `diagnostics` sink protocol に対応する。未対応なら何もしない。
- `Run` / `Experiment` は import しない。

OMMX core 側の責務:

- `run.log_solve(...)` / `run.log_sample(...)` で Solve-scoped `DiagnosticsSink` を作る。
- Adapter が `diagnostics` kwarg または同等の optional protocol に対応している場合だけ sink を渡す。
- diagnostics を media type と annotations を持つ Solve-scoped Attachment として保存する。
- diagnostics Attachment を committed Artifact に含める。
- diagnostics Attachment を要約し参照する OTel span attribute / event を出す。
- Adapter が例外を投げた場合でも、sink に書かれた diagnostics は Solve-scoped Attachment として残す。

diagnostics は 2 層構造にする。

| 層 | 役割 | 例 |
|---|---|---|
| Attachment / Artifact | diagnostic payload の正本 | raw solver log, compressed log, JSON termination report, gap timeline |
| OTel trace | lifecycle、summary、reference | diagnostic recorded event, size, truncation flag, Attachment name, solver status |

diagnostic Attachment の例:

```python
DiagnosticEntry(
    name="solver/scip/log",
    media_type="text/plain; charset=utf-8",
    data=log_bytes,
    annotations={
        "org.ommx.solver.name": "scip",
        "org.ommx.solver.diagnostic.kind": "log",
        "org.ommx.solver.log.truncated": "false",
    },
)
```

対応する OTel event:

```text
event: ommx.solver.diagnostic.recorded
attributes:
  ommx.attachment.name = "solver/scip/log"
  ommx.attachment.media_type = "text/plain; charset=utf-8"
  ommx.solver.name = "scip"
  ommx.solver.diagnostic.kind = "log"
  ommx.solver.diagnostic.size = ...
  ommx.solver.diagnostic.truncated = false
```

Phase 1 は diagnostics payload を Attachment / Artifact に保存し、OTel は summary と reference のみを持つ。OTel Logs signal への本格統合は Phase 2 以降で扱う。

## 6. OTel / Trace / Renderer

Experiment v3 は、記録データ、実行 telemetry、表示、Artifact version を明確に分ける。

| 領域 | 正本 | 例 |
|---|---|---|
| 記録データ | Artifact manifest / layers / aggregate payload | run parameters, metadata, objects, Instance, Solution, SampleSet |
| 実行 telemetry | OTel trace | lifecycle, duration, solver execution, IO, error, attachment / solve event |
| console / notebook 表示 | Trace renderer | text tree, live view, Chrome trace export |
| version / sharing | Artifact manifest | digest, tag, subject lineage, layer descriptors |

`run.log_parameter(...)` の一次効果は Run table の parameter cell 更新である。`run.log_solution(...)` や `run.log_instance(...)` の一次効果は Attachment の追加であり、`run.log_solve(...)` の一次効果は Solve entry の追加である。同時に OTel span event を出すことはできるが、それは「この run で何が記録されたか」を可視化する telemetry であり、データ本体ではない。

### 6.1 Span 階層

OMMX は global `TracerProvider` を暗黙に設定しない。Experiment / Run / builder は active provider がある場合にそれを使い、ない場合は trace capture mode に従う。

Span の基本構造:

| 操作 | Span 名 | 親 |
|---|---|---|
| Experiment 開始 | `ommx.experiment` | active span があれば child、なければ root |
| Run 開始 | `ommx.run` | `ommx.experiment` |
| Adapter solve 実行 | `ommx.solver.solve` | `ommx.run` |
| Adapter sample 実行 | `ommx.solver.sample` | `ommx.run` |
| Attachment / Solve 追加 / Run parameter 更新 | span event | current run / experiment span |
| Artifact commit/build | `ommx.artifact.build` | active span |
| Artifact load | `ommx.artifact.load` | active span |
| Artifact push | `ommx.artifact.push` | active span |

Trace ID は OTel が発行する。OMMX は独自 Trace ID を採番しない。

### 6.2 Attachment / Solve / parameter event

各 `log_*` は Attachment / Solve 追加または Run parameter table 更新の後、可能なら current span に event を追加する。

Event 名:

- `ommx.attachment.added`
- `ommx.solve.recorded`
- `ommx.run.parameter.recorded`

Attachment event attributes:

| Attribute | 内容 |
|---|---|
| `ommx.attachment.owner` | `experiment` / `run` / `solve` |
| `ommx.run.id` | run scope の場合 |
| `ommx.solve.id` | solve scope の場合 |
| `ommx.attachment.name` | Attachment name |
| `ommx.attachment.media_type` | payload media type |
| `ommx.attachment.digest` | commit 後に分かる場合。Build 中は absent でもよい |

Run parameter event attributes:

| Attribute | 内容 |
|---|---|
| `ommx.run.id` | run id |
| `ommx.run.parameter.name` | parameter name |
| `ommx.run.parameter.scalar_type` | `int`, `float`, `string`, `bool`, `null` 等 |

Event は Attachment、Solve entry、または Run parameter cell への reference であり、payload 本体を OTel event attribute に入れない。parameter の small scalar を display 用に入れるかは renderer policy として扱い、正本にはしない。

### 6.3 Trace capture mode

Trace capture mode の候補:

| mode | 動作 |
|---|---|
| `trace="auto"` | デフォルト。provider / collector が設定済みなら trace layer を埋め込む。未設定なら trace layer を省略し、status annotation を残す |
| `trace="required"` | provider / collector が未設定なら setup error |
| `trace=False` | trace layer を生成しない |
| `with_trace()` | low-level builder の明示要求。未設定なら setup error |

`trace="auto"` で trace layer を省略した場合の manifest annotations:

- `org.ommx.trace.status=not_recorded`
- `org.ommx.trace.reason=no_tracer_provider`

既存の notebook / script tracing helper が UX のために provider を install する可能性は別途検討する。ただし Experiment / Artifact build の core path は global provider を勝手に install しない。

### 6.4 Trace layer

Artifact は build-time trace body を専用 layer として持てる。これは batch job や CI のように Artifact 入出力だけで完結する環境で重要である。

Phase 1:

| 項目 | 方針 |
|---|---|
| encoding | OTLP JSON |
| media type | `application/vnd.ommx.trace.otlp+json` |
| payload | OTLP JSON mapping の `ExportTraceServiceRequest` 互換 (`resourceSpans`) |
| 対象 signal | span / span event |
| API | `artifact.get_trace() -> TraceResult` |

Trace layer は Attachment / Run parameter table / Solve entry の代替ではない。parameter / solution / sample set などの本体は Experiment state の物理化戦略に従って保存し、trace layer は実行時系列と logical entry reference を保存する。

### 6.5 Renderer

`MintoLogger` 相当の独立 logger class は作らない。console output は OTel span / event の renderer として実装する。

Phase 1:

- 後から trace を読む renderer のみ。
- `TraceResult.text_tree(style="experiment")` 相当で階層表示する。
- Chrome Trace Event Format は読み出し時に derived format として生成する。

Phase 2:

- scoped streaming renderer を追加する。
- `Experiment(..., live=True)` 相当で opt-in。
- 対象 trace_id だけを購読する scoped processor を、呼び出し側が設定した SDK `TracerProvider` に attach する。
- span end / event を逐次 render する。

Phase 2 は span / event schema を変更せず、同じ OTel signal を読む renderer を増やす形にする。

## 7. Artifact への写像

### 7.1 Manifest identity と Experiment profile

`1 Experiment = 1 Artifact` とするが、Experiment 専用の top-level Artifact media type は初期設計では作らない。既存 Artifact 仕様と同じく、OCI manifest descriptor / transport 上の media type は `application/vnd.oci.image.manifest.v1+json`、manifest の `artifactType` は `application/org.ommx.v1.artifact` のままとする。

Experiment であることは、OMMX Artifact の profile / kind として表す。

| 場所 | 値 |
|---|---|
| OCI manifest descriptor media type | `application/vnd.oci.image.manifest.v1+json` |
| OCI manifest `artifactType` | `application/org.ommx.v1.artifact` |
| OCI manifest `config.mediaType` | `application/org.ommx.v1.experiment.config+json` |
| Experiment config JSON | `status=finished|failed`, Run / Solve structure, Attachment `LayerRef` |
`Artifact.load()` は従来通り OMMX Artifact として読み、`Experiment.load()` は OCI config descriptor の media type で Experiment profile を確認した上で、config blob の Experiment config JSON から immutable Experiment view を復元する。`config.mediaType` が `application/vnd.oci.empty.v1+json` なら v1 互換の通常 Artifact、`application/org.ommx.v1.experiment.config+json` なら Experiment と判定する。Layer annotations は inspector / compatibility 用の補助情報であり、loader が全 layer を scan して意味を推測する設計にはしない。これにより、Experiment は OMMX Artifact family の一種として扱え、既存の Local Registry / archive / remote transport / generic Artifact inspector と互換にできる。

通常の成功 commit は config JSON に `status=finished` を持つ Experiment Artifact として requested tag / ref に publish する。例外終了時に作る failed recovery artifact は config JSON に `status=failed` と recovery marker を持ち、`<registry-id8>.ommx.local/crashed:<local-timestamp>-<nonce>` のような reserved ref に publish する。`Experiment.load(tag)` の通常 UX は requested tag / ref の成功 Artifact を読む。recovery artifact は recovery command / inspector から明示的に扱う。

`OMMX Artifact v3` という media type は導入しない。v3 は SDK / 設計フェーズの名前であり、wire format の互換性境界とは分ける。将来、registry の referrers API などで Experiment だけを `artifactType` で filter したい要求が強くなった場合は、`application/org.ommx.v1.experiment` を追加で許容する余地を残す。ただし初期設計では、top-level は `application/org.ommx.v1.artifact` に統一する。

### 7.2 完全な descriptor list としての manifest

各 committed Artifact manifest は、blob bytes の保存タイミングを表すものではない。`layers[]` には、その時点の Experiment view を復元するために必要な typed payload と aggregate JSON などの descriptor を載せる。

Run / Solve は manifest の子 manifest ではなく、OCI config blob に保存した Experiment config の `runs[]` / `solves[]` から復元する。Manifest `layers[]` は payload descriptor table であり、Experiment config は `LayerRef` でその table を参照する。構造復元の source of truth は config であり、payload descriptor の source of truth は manifest `layers[]` である。

初期設計では、少なくとも以下の aggregate payload を通常の Artifact layer として載せる。

| Layer | 目的 | 備考 |
|---|---|---|
| Run parameter table JSON | run ごとの scalar parameter table | 1 cell = 1 layer にはしない |

これらは manifest annotation で表現しない。Experiment profile / schema は OCI config descriptor の media type で表し、status は config JSON で表す。Run parameter table の本体は layer payload として保存する。

Experiment / Run attachments、Solve input / output、diagnostics は個別 layer または aggregate layer として保存し、Experiment config はそれらを `LayerRef` として参照する。`Experiment.load(...)` は layer annotations を scan せず、Experiment config JSON に保存された `LayerRef` と manifest `layers[]` を合わせて読む。Annotation は generic Artifact inspector や migration compatibility のための redundant metadata として扱う。

Instance / Solution / diagnostics などの payload blob は `log_*` 時点で Local Registry の BlobStore に逐次保存される。commit が行うのは、それらの blob と、Run parameter table JSON など commit 時に materialize する payload を含む最終 Experiment state を seal し、復元に必要な descriptor list を immutable manifest として IndexStore に publish することである。

既存 blob は同じ digest の descriptor として再利用できる。Local Registry では CAS として共有され、remote registry では dedup / mount され得る。一方、archive export では、その Artifact 単体で読めるよう参照 blob を含める。

したがって、複数の Run / Solve が同一 bytes の Instance を参照する場合、logical entity は複数存在してよいが、BlobStore 上の実体は同じ digest の 1 blob に共有される。重複排除の前提は serialized bytes が一致することであり、論理的に同じ Instance でも serialization に timestamp や非決定的 ordering が混ざる場合は別 digest になる。

### 7.3 Layer annotations

Attachment layer は Artifact layer descriptor annotations に以下を持てる。ただしこれらは source of truth ではなく、generic inspector / compatibility / debug 用の補助情報である。Experiment / Run / Solve の所属関係は Experiment config の構造と `LayerRef` で表す。

| Annotation | 必須 | 内容 |
|---|---|---|
| `org.ommx.attachment.name` | optional | user-facing attachment name |
| `org.ommx.codec` | optional | external codec identifier, 必要な場合 |

Attachment の media type は descriptor の `mediaType` field にあるため、annotation として重複保持しない。`run_id` / `solve_id` / `solve role` は config の位置と field 名で分かるため、annotation として重複保持しない。

Run parameter table は、必ずしも 1 cell = 1 layer descriptor にならない。run parameter の key-level metadata は aggregate payload の内部 schema に持たせてよい。Experiment metadata を Attachment として復元するときは media type + name に写像する。

user-defined media type の Attachment は caller / external package が指定した `mediaType` をそのまま使う。OMMX core は unknown media type を拒否せず、必要なら `org.ommx.attachment.name` / `org.ommx.codec` を保持して opaque bytes として扱う。

Experiment の識別子は Artifact の Image Name と同一にする。Experiment name を別メタデータとして持たない。created time、OMMX version などの experiment-level metadata は Experiment config または JSON Attachment に保存する。巨大な metadata は config に載せず Attachment にする。

MINTO 由来の `org.minto.*` annotation は新規書き込みでは使わない。既存 MINTO artifact の import compatibility が必要なら、compat loader が `org.minto.*` を読んで `org.ommx.*` Attachment / Solve model に変換する。

### 7.4 Artifact からの復元

`Experiment.load(...)` は Artifact を読み、root manifest の config descriptor が指す Experiment config JSON から immutable Experiment view を復元する。`layers[]` 全体を scan して Run / Solve / Attachment の意味を推測しない。

復元に必要な invariants:

- config media type は `application/org.ommx.v1.experiment.config+json` とする。
- config は Experiment attachments、Run list、Run ごとの attachments / solves、Run parameter table layer への `LayerRef` を持つ。各 `LayerRef` は、この config blob を所有する OCI Image Manifest の `layers[]` index として解決する。
- run id は 0-based integer とする。ただし復元対象の Run 集合は config の `runs[]` が source of truth であり、欠番を layer scan から補完しない。
- solve id は Run 内の 0-based integer とする。ただし復元対象の Solve 集合は config の `runs[].solves[]` が source of truth であり、欠番を layer scan から補完しない。
- `LayerRef` は manifest `layers[]` の範囲内でなければならない。
- `Solve.input` が参照する layer は OMMX Instance media type、`Solve.output` が参照する layer は OMMX Solution media type を持たなければならない。
- 同一 Experiment / Run 内で同一 `LayerRef`、同一 digest、同一 media type の Attachment が複数箇所から参照されても、committed config では重複を禁止しない。loader は config に書かれた順序と重複を保持する。
- Run parameter table で同一 `(run_id, parameter_name)` が複数値を持つ場合も、committed manifest では重複を禁止し、loader は error にする。
- Run parameter table が config に存在しない run id を参照する場合、loader は error にする。

## 8. Lineage

Artifact lineage は OCI v1.1 `subject` で表す。初期設計では各 Artifact が 0/1 個の parent を持つ single-parent history のみを扱う。複数 child は自然に発生してよい。

| API | 方針 |
|---|---|
| `parent()` | `subject` を読む。0/1 件 |
| `history()` | `subject` chain を root 方向に辿る |
| `diff(other)` | Attachment / Solve list、Run parameter table、layer descriptor を比較する |

`subject` は provenance / lineage 用リンクであり、Artifact 復元に必須の dependency ではない。各 manifest は復元に必要な descriptor list を持つので、単一 Artifact archive は parent chain なしで読める。

保存済み Experiment に run を追加する場合は、loaded `Experiment` をその場で mutable にせず、`with exp.fork(tag=...) as forked:` のように parent Artifact から派生した新しい session / builder を作る。re-enter はこの派生 session を開く操作であり、元 Artifact を変更する操作ではない。正常終了時の自動 commit では新しい manifest を作り、`subject` に parent manifest descriptor を記録する。既存 run の descriptor / blob は再利用し、新しい run で追加された Instance / Solution / diagnostics / aggregate JSON だけが BlobStore に追加される。

同じ仕組みで、派生した Experiment version から run を削除する操作も表現できる。削除は既存 blob を消す操作ではなく、新しい manifest からその run に対応する attachments / solves、Run parameter table row、index entry を省く操作である。元 Artifact は immutable な parent として残り、削除された run の blob は parent からは引き続き参照され得る。物理的な blob 削除は GC の到達可能性解析と retention policy に委ねる。

複数 experiment の統合は lineage merge としては扱わない。必要なら新規 Artifact の Attachment として入力 Artifact digest を列挙する。これは parent ではなく data reference である。

Referrers API を使った child listing は初期必須 API にしない。remote registry compatibility に依存するため、manifest と `subject` だけで完結する parent 方向の走査を先に安定させる。

## 9. Garbage Collection

`ommx artifact gc` 相当の command と、到達可能性解析に必要な API hook を提供する。

GC roots:

- Local Registry refs
- failed recovery manifest refs
- user-specified protected digests
- publish 中の in-flight manifest / ref update
- protected root から辿れる `subject` chain

Local Registry GC:

- IndexStore の manifest / blob records から到達可能性を解析する。
- BlobStore に存在するが IndexStore から参照されない blob は orphan blob として扱う。
- `log_*` 時点で BlobStore に書かれたが、成功 commit または failed recovery manifest に到達しなかった orphan blob / autosave metadata を削除候補にする。
- failed recovery manifest は retention policy が許す間 GC root として扱い、期限後に削除候補にする。
- 派生 Experiment version で run が削除されても、parent Artifact が root または protected subject chain から到達可能なら、その run の blob は保持される。storage reclaim は parent lineage の retention / pruning policy と組み合わせて行う。
- publish 途中の blob を誤削除しないよう grace period を置く。
- IndexStore record があるが BlobStore に bytes がない場合は corruption として report する。

Archive / exported OCI directory:

- manifest / index / explicit root digest から到達可能 blob を辿る。
- 未到達 blob を削除候補にする。

Remote registry:

- registry 実装ごとに deletion / retention policy が異なるため、初期設計では到達可能性解析と削除候補の列挙を優先する。
- 実削除は registry capability を検出できる場合だけ行う。

GC は data model を変えない。完全な descriptor list を持つ manifest、digest primary、single-parent lineage、Attachment / Solve model と Run parameter table とは独立した maintenance operation とする。

## 10. 現在の実装範囲

PR #886 時点で、Experiment / Artifact v3 提案のうち、Rust core と Python から使う最小の Experiment UX は実装済みである。この節は、本ファイルが一時ドキュメントである間に、未実装 Proposal と実装済み仕様を混同しないための作業メモである。

### 10.1 Rust core

実装済み:

- `ommx::experiment::{Experiment, Run, SealedExperiment, SealedRun, Solve}`。
- `Experiment::new(Name)` / `Experiment::with_registry(&LocalRegistry, Name)` / `Experiment::with_temp_local_registry(...)`。
- `Name::Named(ImageRef)` と `Name::Anonymous`。anonymous Experiment は `<registry-id8>.ommx.local/experiment:<timestamp>-<nonce>` に解決する。
- `Experiment::log_attachment` / `log_json` / `log_instance` / `log_solution` / `log_sample_set`。
- `Run::log_parameter` / `log_attachment` / `log_json` / `log_instance` / `log_solution` / `log_sample_set` / `log_finished_solve_result`。
- `Run::finish(self)`。Rust 側では finish が `Run` を消費するため、終了済み Run に後から Attachment / parameter を追加する経路は型として存在しない。
- `Experiment::commit(self) -> SealedExperiment<'reg>`。Rust 側では commit が `Experiment` を消費するため、commit 後 mutation は型として存在しない。
- `SealedExperiment::from_artifact(LocalArtifact)` による config-based load。
- `SealedExperiment::experiment_attachments()`、`runs()`、`run(id)`、`run_parameter_cells()`。`SealedRun::solves()` から Solve input / output / parameters を参照できる。

Rust core の `Experiment<'reg>` は `&'reg LocalRegistry` を持つ。`Run<'exp, 'reg>` は親 Experiment を immutable borrow し、Run local state を持つ。close 済み Run は `RunEntry` として `Experiment` state の `BTreeMap<run_id, RunEntry>` に保存され、commit 時に `run_id` 順で materialize される。

`log_*` は payload を即 Local Registry の BlobStore に保存し、in-memory には `StoredDescriptor` を保持する。同じ bytes は CAS blob として物理的に共有されるが、Attachment descriptor list と Solve list はユーザーの log call 順と重複を保持する。

### 10.2 Artifact への写像

実装済みの wire shape:

| 要素 | 値 / 方針 |
|---|---|
| OCI manifest descriptor media type | `application/vnd.oci.image.manifest.v1+json` |
| OCI manifest `artifactType` | `application/org.ommx.v1.artifact` |
| OCI config media type | `application/org.ommx.v1.experiment.config+json` |
| Experiment config `status` | 成功 commit は `finished` |
| Run parameter layer media type | `application/org.ommx.v1.experiment.run-parameters+json` |

Experiment config JSON は、Experiment attachments、Run list、Run ごとの attachments / solves、Run parameter table への `LayerRef` を持つ。`SealedExperiment::from_artifact` は root manifest の config media type を見て Experiment profile を判定し、config blob と manifest `layers[]` を合わせて構造を復元する。`layers[]` 全体を scan して Run / Solve / Attachment の意味を推測しない。

Load 時の検証:

- config media type が `application/org.ommx.v1.experiment.config+json` である。
- config `status` は `finished` である。`failed` / `crashed` などの recovery artifact は通常の `SealedExperiment` として読まない。
- config に書かれた `LayerRef` は manifest `layers[]` の範囲内である。
- Experiment / Run attachments として復元する layer は、`org.ommx.attachment.name` annotation を持つ。
- Solve input は Instance media type、Solve output は Solution media type を持つ。
- `LayerRef` が指す blob はその Local Registry に存在し、size が一致する。
- Run parameter table が config に存在しない run id を参照する場合は error にする。

Attachment layer annotations:

| Annotation | 内容 |
|---|---|
| `org.ommx.experiment.space` | `experiment` / `run` |
| `org.ommx.experiment.run_id` | run space のみ |
| `org.ommx.attachment.name` | Attachment name |

Run parameter table は column-oriented JSON として aggregate layer に保存する。column type は commit 時に確定し、`int64 -> float64` の昇格だけを許す。それ以外の型混在は commit error とする。欠損 cell は JSON `null` ではなく、該当 `(run_id, parameter_name)` entry が存在しないことで表す。

### 10.3 Python SDK

実装済み:

- `ommx.experiment.Experiment` / `Run` / `SealedRun` / `Solve`。
- `Experiment()` は default Local Registry 上に anonymous Experiment を作る。名前を指定する場合は `Experiment(image_name)` とする。
- `Experiment.with_temp_local_registry()` は Python test / example 用の一時 Local Registry を持つ Experiment を作る。一時 registry は、その Experiment から commit された `Artifact` や `Experiment.from_artifact(...)` でロードされた Experiment が生きている間保持される。
- `with Experiment(...) as exp:` は正常終了時に自動 commit する。例外終了時は commit しない。
- `with exp.run() as run:` は正常終了時に Run を finish する。例外終了時は Run を abandon し、Experiment state に反映しない。
- `Experiment.commit() -> Artifact`。
- `Experiment.artifact` は commit 後だけ available。commit 前アクセスは error。
- `Experiment.load(image_name)` は default Local Registry の committed Experiment Artifact を読む。
- `Experiment.from_artifact(artifact)` は既存 `Artifact` を committed Experiment として解釈する。
- `Experiment.experiment_attachments` は Experiment-space Attachment descriptors を返す。
- `Experiment.runs` は `SealedRun` list を返す。
- `SealedRun.attachments` は Run-space Attachment descriptors を返す。
- `SealedRun.solves` は `Solve` list を返し、Solve input / output descriptors と Solve parameters を参照できる。
- `Run.log_solve(adapter, instance, **kwargs)` は adapter を呼び、caller が渡した Instance を Solve input、返された Solution を Solve output、adapter 名と `json.dumps` した kwargs を Solve parameters として記録する。kwargs は Run parameter table には入れない。
- `Experiment.run_parameters_df()` は pandas DataFrame を返す。parameter を持たない完了済み Run も index row として残す。

Python binding では、Rust lifetime を Python object model に露出できないため、`ExperimentDyn` / `RunDyn` / `LocalArtifactDyn` / `LocalRegistryHandle` を使う。`ExperimentDyn` は registry owner と lifecycle state を持ち、open Run count を追跡する。open Run が残っている間の commit は error にする。`RunDyn` は drop された live Run を abandon し、`finish(self)` では Run local state を parent Experiment に書き戻す。

`PyRun` は Python object が残ったまま Rust の `RunDyn::finish(self)` を呼ぶ必要があるため、内部に `Option<RunDyn>` を持つ。`Some` は open Run、`None` は finish / abandon 済みの handle を表す。

### 10.4 テスト済みの重要条件

Rust tests:

- run id 採番、複数 Run の同時 open、finish による close 済み Run の記録。
- log 時点で BlobStore に payload が保存されること。
- Attachment descriptor list が保存順と重複を保持すること。
- byte-identical Attachment が digest を共有しつつ、descriptor / Attachment としては別に保持されること。
- scalar parameter の materialization、`int64 -> float64` 昇格、型混在 error、非有限 float rejection。
- committed Artifact の config / `LayerRef` / Attachment layer annotation / aggregate layer。
- Rust core の `Run::log_finished_solve_result` が、solve 実行は行わず、渡された input / output から Solve entry を作り、`LayerRef` で参照し、Solve parameters を Run parameter table から分離すること。
- config-based Experiment load、`status != finished` の rejection、Run parameter table が未知 run id を参照する場合の rejection。
- dynamic Experiment が一時 Local Registry を保持し、open Run を持つ commit を拒否し、unfinished Run drop を abandon として扱うこと。

Python tests:

- `Experiment.with_temp_local_registry()` を使った Experiment / Run / Attachment / parameter / commit / load。
- Experiment context manager 正常終了時の auto commit。
- Experiment context manager 例外終了時に commit しないこと。
- Run context manager 例外終了時に finish しないこと。
- temp registry が Experiment drop 後も derived Artifact / loaded Experiment とともに生きること。
- Python int が `int64` を超える場合の error。
- `run_parameters_df()` が parameterless completed Run を index row として保持すること。
- `Run.log_solve` が 1 Run 内の複数 Solve を `solve_id` 順に保存し、kwargs JSON 文字列を Solve parameters として復元すること。

## 11. 残り設計事項

### 11.1 Failed recovery / autosave metadata

Blob 本体は既に `log_*` 時点で BlobStore に逐次保存される。一方、process crash や Python context manager 例外時に、どの Attachment / Run / Solve / parameter がどの blob に対応していたかを復元する metadata は未実装である。

残作業:

- uncommitted Experiment state の autosave metadata format。
- 例外終了時に `<registry-id8>.ommx.local/crashed:<timestamp>-<nonce>` のような reserved ref へ recovery artifact を publish するか。
- recovery artifact の config `status` と、通常の `Experiment.load(...)` からは読まない UX。
- recovery command / inspector の API。
- orphan blob と autosave metadata の retention policy。

### 11.2 Fork / lineage / run deletion

`subject` を使った parent linkage、`Experiment.fork(...)`、`parent()`、`history()`、`diff(other)` は未実装である。保存済み Experiment に Run を追加する操作は、既存 Artifact を mutable に戻すのではなく、loaded Experiment view から新しい unsealed session を作り、parent を `subject` に持つ child Artifact として commit する。

残作業:

- forked session の名前 / tag 解釈。
- parent の Attachment / Run / Solve / parameter を child state へどう投影するか。
- new run id の採番規則。
- run deletion を「child manifest から run を省く操作」として扱う API。
- parent lineage と GC retention の関係。

### 11.3 Adapter execution / diagnostics

`run.log_solve(...)` は、Run 内の Solve list、Solve input / output の `LayerRef`、Solve parameters への kwargs JSON 文字列保存まで実装済みである。`run.log_sample(...)`、diagnostics sink protocol、adapter diagnostics Attachment は未実装である。

残作業:

- `log_sample` の SampleSet-oriented Solve / Sample entry。
- backend solver 名、solver status、elapsed time など Solve scoped metadata の schema。
- `DiagnosticsSink` / `DiagnosticCollector` / `DiagnosticEntry` の型と media type validation。
- Adapter が diagnostics protocol に対応しているかの検出方法。
- solver native log、termination report、gap timeline などをどの media type と Attachment 名で保存するか。
- OTel event には diagnostics payload 本体ではなく Attachment / Solve reference と summary だけを持たせる実装。

### 11.4 Run attributes / environment

Run status、elapsed time、実行環境 OS / package versions / backend solver version などは未実装である。この PR では Run attributes aggregate payload を導入していない。

残作業:

- Run status を Artifact data model に持つか、OTel span status だけに寄せるか。
- 実行環境情報を Run の属性として保存する場合の schema。
- 環境情報を Attachment として持たせるか、config / aggregate payload として持たせるか。
- Python / Rust で取得できる情報の差分。

### 11.5 OTel trace / renderer

Experiment core はまだ OTel span / event / trace layer を生成しない。`trace="auto"` / `trace="required"` / `trace=False`、OTLP JSON trace layer、post-hoc renderer は未実装である。

残作業:

- Experiment / Run / solver / artifact build / load / push span schema。
- `ommx.attachment.added` / `ommx.solve.recorded` / `ommx.run.parameter.recorded` events。
- global `TracerProvider` を暗黙に install しないことの tests。
- trace layer media type と `artifact.get_trace()`。
- text tree / Chrome trace export renderer。

### 11.6 GC

Local Registry GC は未実装である。BlobStore に存在するが IndexStore ref / manifest から到達できない orphan blob は、現時点では残り得る。

残作業:

- Local Registry refs、failed recovery refs、protected digests、subject chain からの到達可能性解析。
- orphan blob / unreferenced manifest / autosave metadata の dry-run report。
- retention / grace period。
- archive / OCI directory の到達可能性解析。
- remote registry は deletion capability を検出できる範囲で dry-run を優先する。

### 11.7 Legacy MINTO import

`org.minto.*` annotation を読む compatibility loader は未実装である。必要なら、OMMX core に持つか migration tool として外に出すかを決める。

## 12. 項目別実装ステータス

実装順を大きな工程名として固定せず、設計項目ごとに現在の状態を記録する。状態は本ファイル上の作業メモであり、実装が進んだら Sphinx docs / Rust API reference / Python API reference / issue に分解していく。

| 項目 | 状態 | 現状 | 残作業 |
|---|---|---|---|
| Artifact manifest / Local Registry v3 | 実装済み | SQLite-backed Local Registry、BlobStore、Image Manifest、`StoredDescriptor`、`SealedArtifact`、`UnsealedArtifact` は実装済み | 本ファイルからは詳細を削除済み。正式 docs / rustdoc を正本にする |
| Experiment Artifact profile | 実装済み | `config.mediaType=application/org.ommx.v1.experiment.config+json` で Experiment を判定する。top-level `artifactType` は `application/org.ommx.v1.artifact` のまま | Wire format の仕様文書化 |
| Experiment config JSON | 実装済み | `status`、Experiment attachments、Run list、Run-scoped attachments、Run ごとの Solve list、Run parameter table への `LayerRef` を保存する | schema evolution policy、unknown field / versioning の扱い |
| Config status validation | 実装済み | `SealedExperiment::from_artifact` は `status=finished` だけを通常の sealed Experiment として読む | failed / crashed recovery artifact の別 loader |
| Rust `Experiment<'reg>` / `Run<'exp, 'reg>` lifecycle | 実装済み | Rust lifetime で registry / Experiment / Run lifetime を表す。`Run::finish(self)` と `Experiment::commit(self)` は handle を消費する | 追加なし。API reference へ移す |
| Rust `SealedExperiment` view | 実装済み | Config と manifest `layers[]` から Experiment-space Attachments、Run list、Run-scoped Attachments、Run-scoped Solves、Run parameter cells を復元する | higher-level table / summary views |
| Dynamic lifetime handles | 実装済み | `ExperimentDyn` / `RunDyn` / `LocalArtifactDyn` / `LocalRegistryHandle` で Python 向けに owner を保持する | 名前 / rustdoc の整理、必要なら public surface の最小化 |
| Python `ommx.experiment` module | 実装済み | `Experiment` / `Run` / `SealedRun` を公開。context manager、commit、load、from_artifact、temp registry を提供する | user guide / examples |
| Python temporary Local Registry | 実装済み | `Experiment.with_temp_local_registry()` が temp registry owner を保持し、derived Artifact / loaded Experiment に寿命を伝播する | なし。API docs へ移す |
| Experiment / Run Attachment logging | 実装済み | `log_attachment` / `log_json` / `log_instance` / `log_solution` / `log_sample_set` を Experiment space / Run space に追加する | user-defined media type の docs、codec convention |
| Attachment descriptor list semantics | 実装済み | Attachment descriptor list は保存順と重複を保持する。dedup は BlobStore の digest 単位に限定する | Attachment list / Solve list 表示方針 |
| Run parameter table | 実装済み | `bool` / `int64` / `float64` / `string` の scalar cell を column-oriented JSON に materialize する。`int64 -> float64` だけ昇格する | Arrow / Parquet など別 physical format は必要が出るまで見送り |
| Python `run_parameters_df()` | 実装済み | pandas DataFrame を返し、parameter を持たない完了済み Run も index row として保持する | column dtype の細部、summary / filtering API |
| Open Run commit rejection | 実装済み | Python dynamic handle では open Run count を持ち、open Run が残る commit を拒否する | error message / docs |
| Run exception handling in Python | 実装済み | `with run:` の例外終了では Run を abandon し、Experiment state へ書き戻さない | 例外時 diagnostics / partial attachments の recovery policy |
| Experiment context manager auto commit | 実装済み | Python `with Experiment(...)` は正常終了時に auto commit し、例外終了時は commit しない | failed recovery artifact と接続するか |
| Failed recovery / autosave metadata | 未実装 | Blob 本体は log 時点で保存されるが、uncommitted state を復元する metadata はない | metadata format、reserved ref、recovery command、retention policy |
| Recovery artifact | 未実装 | `status != finished` は通常 load から拒否するところまで実装済み | `crashed:<timestamp>-<nonce>` ref、`status=failed` config、明示 loader / inspector |
| Fork / lineage | 未実装 | `subject` を使う方針のみ | `parent()`、`history()`、`Experiment.fork(...)`、child commit、run id 採番 |
| Run deletion as child projection | 未実装 | 設計方針のみ | child Artifact から run descriptors / parameter row を省く API |
| Adapter `log_solve` / `log_sample` | 部分実装 | `Run.log_solve` は Python で実装済み。adapter 名と kwargs JSON 文字列は Solve parameter、input / output は Solve entry の `LayerRef` として保存する | `log_sample`、backend solver 名 / status / elapsed time、API 名の整理 |
| Diagnostics sink protocol | 未実装 | OTel ではなく Attachment / Artifact を正本にする方針のみ | `DiagnosticsSink` / `DiagnosticCollector` / `DiagnosticEntry`、media type、truncation / compression policy |
| Run attributes / environment | 未実装 | この PR では aggregate payload を導入しない方針 | status、elapsed time、OS / package / solver version の schema |
| OTel span / event integration | 未実装 | schema 方針のみ | Experiment / Run / solver / artifact operation spans、attachment / solve / parameter events |
| Trace layer | 未実装 | OTLP JSON layer 方針のみ | media type、capture mode、`artifact.get_trace()` |
| Trace renderer | 未実装 | renderer を OTel signal の derived view とする方針のみ | text tree、Chrome trace export、streaming renderer |
| GC | 未実装 | GC root / reachability 方針のみ | Local Registry dry-run / report / delete、orphan blob retention、archive / remote registry handling |
| Legacy MINTO import | 未実装 | 新規書き込みでは `org.minto.*` を使わない方針 | compatibility loader を core に持つか migration tool にするか |
| Documentation integration | 部分実装 | API reference は stubgen で更新済み。本ファイルは一時文書 | Sphinx guide、Rust API docs、wire format docs へ分割し、本ファイルを削除する |
