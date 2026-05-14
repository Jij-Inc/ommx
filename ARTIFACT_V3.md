# OMMX Experiment / Artifact v3 提案

OMMX v3 における Experiment / Run / Record / Trace / Lineage / GC の未実装領域をまとめる提案。

本ファイルは開発中の一時文書である。実装完了後は削除し、内容を通常の Sphinx documentation / API reference / module rustdoc に統合する。既に実装済みの Artifact manifest format、Local Registry、archive / remote transport の移行ログは本書では扱わない。必要な前提は `rust/ommx/doc/artifact_design.md` と `ommx::artifact::local_registry` の rustdoc を参照する。

本書の主眼は、MINTO が提供していた実験管理 UX を OMMX-owned な機能として再設計し、記録データ、実行 telemetry、Artifact version、lineage を一貫したモデルに落とすことである。

## 1. 目標 UX

OMMX が提供する実験管理機構の中心 UX は、Experiment を実験全体の mutable session として扱い、正常終了時に immutable Artifact として commit することである。API 名は提案であり、実装時に Python / Rust の慣習に合わせて調整してよい。

```python
from ommx.experiment import Experiment

with Experiment("scip_reblock115", trace="auto", tag="scip_reblock115:latest") as exp:
    exp.log_metadata("dataset", "miplib2017")
    exp.log_metadata("source_problem", "reblock115")

    for formulation in formulations:
        with exp.run() as run:
            run.log_parameter("formulation", formulation.name)
            run.log_object("disabled_constraints", formulation.disabled_constraints)

            candidate = formulation.apply(instance)
            run.log_instance("candidate", candidate)

            solution = run.log_solve(
                "scip",
                OMMXPySCIPOptAdapter,
                candidate,
                time_limit=1.0,
            )

artifact = exp.artifact
table = exp.get_run_table()
trace = artifact.get_trace()
```

保存済み Experiment に Run を追加する場合は、既存 Artifact を mutable に戻さず、loaded Experiment view から fork した新しい session を開く。

```python
from ommx.experiment import Experiment

exp = Experiment.load("scip_reblock115:latest")

with exp.fork(tag="restarted") as forked:
    with forked.run() as run:
        run.log_parameter("formulation", "new_relaxation")
        run.log_instance("candidate", candidate)
        run.log_solution("result", solution)

child = forked.artifact
assert child.parent() == exp.artifact
```

この UX で保証したいこと:

- experiment 全体のデータと run 固有のデータを明確に分離できる。
- dataset / source problem / baseline configuration は experiment space に metadata / object として記録できる。
- run ごとに変わる formulation / solver kwargs などの parameter を Run table の列として記録し、後から比較できる。
- Instance、Solution、SampleSet、object、diagnostics は Experiment / Run のどちらにも Record として記録できる。
- parameter は Record ではなく、Experiment が持つ Run 一覧に付随する表形式データとして扱う。
- `log_*` は logger API ではなく、Experiment state への記録 API である。
- `Run` の context manager は Run lifecycle を閉じ、status / elapsed time / diagnostics / trace を Experiment state に反映する。
- `Experiment` の context manager は mutable session の lifecycle を閉じ、正常終了時に immutable Artifact version を自動 commit する。
- Artifact は digest primary、tag は mutable alias として共有に使う。
- Artifact を archive / Local Registry / remote registry から reload して、table と trace を再構成できる。
- 保存済み Artifact から派生 session を開き、新しい Run を追加して child Artifact として commit できる。
- 実験途中で process が落ちても、BlobStore に退避済みの payload bytes は即座には消えない。Experiment としての復元は、対応する autosave metadata が残っている範囲で best-effort に行う。

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
| Build | mutable。BlobStore-backed autosave と内部 metadata を持つ | `Experiment`, `Run`, builder |
| Seal | immutable Artifact version を作る | `commit()` / context manager exit |
| View | read-only | `Artifact`, loaded `Experiment`, table / trace view |

永続化済み Artifact を更新する API は作らない。既存 Artifact から新しい version を作る場合は、parent を lineage として記録する。

### 3.2 Experiment context manager

`with Experiment(...)` は mutable session の lifetime を表す。

- 正常終了時は自動 commit する。
- 例外終了時は requested tag への成功 commit は行わない。
- 例外終了時は best-effort で failed recovery manifest を reserved ref に publish し、BlobStore に書き込まれた blob と autosave metadata を recovery / GC 用に到達可能にする。
- block 内で明示 `commit()` 済みの場合、`__exit__` の commit は no-op にする。
- commit 後の `log_*` / `run()` / `fork()` は禁止する。
- `exp.artifact` は commit 後に available とし、commit 前アクセスは error にする。
- context manager を使わない場合は、明示 `exp.commit(tag=...)` を呼ぶ。

`1 Experiment commit = 1 Artifact manifest` をデフォルトとする。`Run` 終了ごとに manifest を切る挙動は初期設計では提供しない。必要になった場合も opt-in とし、通常の比較 UX を複雑にしない。

Run は Artifact sub manifest ではなく、Experiment manifest 内の `run_id` で束ねられる logical entity とする。Run の保存実体は、Run parameter table の row、Run attributes の row、`space=run` / `run_id=N` annotation を持つ Record layer、trace layer 内の `ommx.run` span である。

### 3.3 Run context manager

`with exp.run() as run:` は Run lifecycle を表す。

- Run 開始時に `run_id` を採番する。
- `run.log_parameter(...)` は Run table の cell を更新する。
- `run.log_instance(...)` / `run.log_solution(...)` / `run.log_sample(...)` は Run space の Record を追加する。
- 正常終了時は Run status を `finished` にし、elapsed time と実行環境属性を Run attributes に反映する。
- 例外終了時は Run status を `failed` にし、例外 summary と取れる範囲の diagnostics を反映する。
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

orphan blob だけでは、どの Experiment / Run / Record に属していたかを復元できない。Experiment session として復元するには、`run_id`、Record kind / name、blob digest、Run parameter table、Run attributes などを結ぶ autosave metadata が必要である。この metadata が残っている範囲では recovery command が session を再構成できる。metadata がない blob は単なる orphan blob として扱い、grace period 後に GC 対象にする。

例外終了を検知できた場合は、成功 Artifact と同じ tag には publish せず、`status=failed` / `recovery=true` 相当の annotation を持つ recovery manifest を作る。この manifest はその時点で分かっている Record descriptors、Run parameter table、Run attributes、autosave metadata への link を含み、reserved ref で Local Registry に保持する。reserved ref は既存 anonymous artifact と同じ形式に合わせ、例えば `<registry-id8>.ommx.local/crashed:<local-timestamp>-<nonce>` とする。これにより、通常の共有 ref は進めずに、失敗した Experiment の途中成果だけを recovery command から辿れる。

recovery manifest の publish 自体に失敗した場合は、BlobStore には orphan blob だけが残り得る。この場合は自動的な Experiment 復元はできず、GC の grace period 内に low-level inspection する程度に留まる。

autosave の内部 metadata format は user-facing compatibility surface にしない。directory layout compatibility より、復元可能性と final Artifact semantics を優先する。commit されずに残った autosave metadata や、どの manifest からも到達しない blob は GC の対象になる。

### 3.5 Fork session

保存済み Experiment に Run を追加する操作は、既存 Artifact の再オープンではなく、loaded Experiment view から fork した新しい mutable session として扱う。

forked session では parent に含まれる既存 Record、Run parameter table、Run attributes は読み取り可能な初期 state として見える。ただし parent Experiment / Artifact 自体は immutable であり、変更は child Artifact の manifest にだけ反映される。

新しい Run は既存 `run_id` と衝突しない id を割り当てる。正常終了時の自動 commit では parent を `subject` に持つ child manifest を作る。例外終了時は child Artifact を commit しない。autosave metadata による recovery は forked session を復元するためのものであり、parent Artifact を変更するものではない。

## 4. Experiment state model

Experiment state は以下からなる。

| 要素 | 内容 |
|---|---|
| Record set | Experiment / Run の名前付き payload |
| Run parameter table | `run_id` と parameter name を key にした scalar table |
| Run attributes | `run_id` を key にした structured attributes |

汎用保存抽象は user-facing concept として導入しない。API / loader から見える単位を Record、Run parameter table、Run attributes に分ける。

### 4.1 Experiment space / Run space

OMMX では `log_global_*` という命名は採用しない。Experiment object に対する `exp.log_*` は experiment space に、Run object に対する `run.log_*` は run space に記録する。保存先は receiver で決まり、暗黙の context では決めない。

ただし `parameter` は Record kind ではなく Run parameter table の列データであり、Experiment には `exp.log_parameter` を持たせない。Experiment 全体に属する dataset、source problem、baseline、analysis context は `metadata` または `object` として扱う。

```python
exp.log_metadata("dataset", "miplib2017")      # experiment space

with exp.run() as run:
    run.log_instance("candidate", instance)    # run space
    run.log_parameter("seed", 0)               # run table parameter
    run.log_solution("result", solution)       # run space
```

### 4.2 Record

Record は API / loader から見える名前付き payload であり、実装上の blob 所有単位を意味しない。各 Record は概念的に space、kind、name、content、media type、annotations を持つ。

| Field | 内容 |
|---|---|
| `space` | `experiment` または `run` |
| `run_id` | run space の場合のみ必須 |
| `kind` | `metadata`, `object`, `instance`, `solution`, `sampleset`, `diagnostic`, `media` |
| `name` | space + kind 内の user-facing key |
| `content` | scalar value、serialized bytes、または blob descriptor への参照 |
| `media_type` | Artifact layer media type。`media` Record では user / external package が指定する |
| `annotations` | Artifact descriptor annotations に投影される metadata |

Record kind は space によって制限しない。`metadata`, `object`, `instance`, `solution`, `sampleset`, `diagnostic`, `media` は Experiment space / Run space のどちらにも置ける。例えば全 run で共有する source Instance は Experiment space に、実際に各 run で解いた candidate Instance は Run space に置く。

対応する Record kind:

| Kind | Payload | 備考 |
|---|---|---|
| `metadata` | JSON | dataset name / source problem id / system metadata などの小さな context。構造化 config は `object` を優先 |
| `object` | JSON | JSON serializable dict / list 等 |
| `instance` | `ommx.v1.Instance` bytes | public API は `ommx.v1` |
| `solution` | `ommx.v1.Solution` bytes | table summary を持つ |
| `sampleset` | `ommx.v1.SampleSet` bytes | table summary を持つ |
| `diagnostic` | JSON または bytes | solver / adapter diagnostic evidence |
| `media` | 任意の bytes + user-specified media type | user / external package が所有する opaque payload |

`media` Record は、OMMX core が schema を知らない user-defined payload の escape hatch とする。caller は bytes と `media_type`、必要なら codec identifier や annotations を指定できる。OMMX core は unknown media type を decode せず、digest / size / media type / annotations を Artifact descriptor として保持する。

OMMX core は `jijmodeling` を import しない。domain-specific problem storage は external package が `media_type` と codec を登録して提供する。例えば `jijmodeling` の model payload は、`jijmodeling` package が media type / codec を所有し、OMMX には `media` Record として渡す。OMMX は descriptor を保持するだけで、parse / validation / round-trip guarantee はその media type owner の責務にする。

### 4.3 Run parameter table

Run parameter table は Record とは別に、`run_id` と parameter name を key にした scalar table として持つ。

```python
run.log_parameter("timelimit", 1.0)
run.log_parameter("seed", 0)
```

この 2 つは論理的には別 parameter cell である。ただし物理的には、run ごとの parameter aggregate JSON または Experiment index JSON にまとめて保存してよい。

物理化候補:

| 対象 | 候補 | 備考 |
|---|---|---|
| run parameter | Run ごとの parameter aggregate JSON または Experiment index JSON | `get_run_table()` の入力になる |
| adapter kwargs | run parameter として aggregate JSON に含める | scalar kwargs のみ。Instance などは別 Record |
| table index | Experiment index JSON | table 再構成を速くするための derived payload として持てる |

この方針では、`parameter` は API / analysis 上は key 単位で扱えるが、Record ではなく Artifact layer の最小単位でもない。`Instance` や `Solution` のような大きな typed payload は Record として即 blob 化しやすいが、scalar parameter は commit 時に Run table payload として materialize する方が manifest / blob 数を抑えられる。

### 4.4 Run attributes

Run attributes は `run_id` を key にした structured attributes である。初期設計では Run status、elapsed time、実行環境属性を含む。

実行環境は Record ではなく Run attribute として保存する。OTel `Resource` はその投影であり、情報本体ではない。

Run は後から追加実行できるため、実行環境は Experiment 全体に固定できない。Run の実行環境属性は、各 Run がどの OS / runtime / package versions / adapter version で実行されたかを表す。Experiment 作成時の SDK / host 情報を残す必要がある場合は、実行環境ではなく provenance metadata として保存する。

保存対象:

- OS / platform
- host / CPU / memory
- process / Python / Rust runtime
- package versions
- container / CI metadata, 取れる場合
- OMMX / adapter version

OTel Resource へ写す属性は standard semantic conventions を優先する。標準属性で表現できない OMMX 固有情報だけを `ommx.*` namespace に置く。同じ意味の値を標準属性と `ommx.*` に二重記録しない。

### 4.5 Build phase と Seal phase

Build phase では同じ `(space, run_id, kind, name)` に対する Record upsert、同じ `(run_id, parameter_name)` に対する parameter upsert、同じ run attribute key に対する upsert を許容する。

Seal / commit phase では最終 Experiment state を復元できる aggregate payload と descriptor set に固定する。Committed manifest では Record、parameter、Run attribute の重複を残さない方針を基本とする。

View phase の `Artifact` / loaded `Experiment` は immutable とし、追加や更新は `fork()` から別 Artifact を作る。

## 5. Adapter execution and diagnostics

### 5.1 Adapter 実行

OMMX では Adapter API が `SolverAdapter.solve(...)` / `SamplerAdapter.sample(...)` として標準化されているため、主要 UX は `run.log_solve(...)` / `run.log_sample(...)` にする。任意 callable を包む generic solver logging API は提供しない。

目標:

- `log_solve` は `SolverAdapter.solve(...)` を呼び、`Solution` を run space に記録する。
- `log_sample` は `SamplerAdapter.sample(...)` を呼び、`SampleSet` を run space に記録する。
- adapter name / backend solver name を run metadata または Run attributes として記録する。
- scalar kwargs を Run table の parameters として記録する。
- `Instance` kwargs は、その run で実際に解いた candidate Instance として記録できる。全 run で本当に共有される巨大な Instance だけは、experiment space Record への明示的な reference を使えるようにする。
- 実行時間は OTel span と Run attributes に記録し、Artifact に保存する結果 payload の正本にはしない。
- adapter が返す diagnostics を Artifact-backed evidence として記録する。

```python
with exp.run() as run:
    solution = run.log_solve(
        "scip",
        OMMXPySCIPOptAdapter,
        instance,
        time_limit=1.0,
    )

with exp.run() as run:
    sample_set = run.log_sample(
        "openjij_sa",
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

- `run.log_solve(...)` / `run.log_sample(...)` で Run-scoped `DiagnosticsSink` を作る。
- Adapter が `diagnostics` kwarg または同等の optional protocol に対応している場合だけ sink を渡す。
- diagnostics を `media` / `object` / `diagnostic` Record として保存する。
- diagnostics Record を committed Artifact に含める。
- diagnostics Record を要約し参照する OTel span attribute / event を出す。
- Adapter が例外を投げた場合でも、sink に書かれた diagnostics は Run-scoped Record として残す。

diagnostics は 2 層構造にする。

| 層 | 役割 | 例 |
|---|---|---|
| Record / Artifact | diagnostic payload の正本 | raw solver log, compressed log, JSON termination report, gap timeline |
| OTel trace | lifecycle、summary、reference | diagnostic recorded event, size, truncation flag, Record name, solver status |

diagnostic Record の例:

```python
DiagnosticEntry(
    name="solver/scip/log",
    kind="solver_log",
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
  ommx.record.kind = "media"
  ommx.record.name = "solver/scip/log"
  ommx.solver.name = "scip"
  ommx.solver.diagnostic.kind = "log"
  ommx.solver.diagnostic.size = ...
  ommx.solver.diagnostic.truncated = false
```

Phase 1 は diagnostics payload を Record / Artifact に保存し、OTel は summary と reference のみを持つ。OTel Logs signal への本格統合は Phase 2 以降で扱う。

## 6. OTel / Trace / Renderer

Experiment v3 は、記録データ、実行 telemetry、表示、Artifact version を明確に分ける。

| 領域 | 正本 | 例 |
|---|---|---|
| 記録データ | Artifact manifest / layers / aggregate payload | run parameters, run attributes, metadata, objects, Instance, Solution, SampleSet |
| 実行 telemetry | OTel trace | lifecycle, duration, solver execution, IO, error, record event |
| console / notebook 表示 | Trace renderer | text tree, live view, Chrome trace export |
| version / sharing | Artifact manifest | digest, tag, subject lineage, layer descriptors |

`run.log_parameter(...)` の一次効果は Run table の parameter cell 更新である。`run.log_solution(...)` や `run.log_instance(...)` の一次効果は Record の追加である。同時に OTel span event を出すことはできるが、それは「この run で何が記録されたか」を可視化する telemetry であり、データ本体ではない。

### 6.1 Span 階層

OMMX は global `TracerProvider` を暗黙に設定しない。Experiment / Run / builder は active provider がある場合にそれを使い、ない場合は trace capture mode に従う。

Span の基本構造:

| 操作 | Span 名 | 親 |
|---|---|---|
| Experiment 開始 | `ommx.experiment` | active span があれば child、なければ root |
| Run 開始 | `ommx.run` | `ommx.experiment` |
| Adapter solve 実行 | `ommx.solver.solve` | `ommx.run` |
| Adapter sample 実行 | `ommx.solver.sample` | `ommx.run` |
| Record 追加 / Run parameter 更新 | span event | current run / experiment span |
| Artifact commit/build | `ommx.artifact.build` | active span |
| Artifact load | `ommx.artifact.load` | active span |
| Artifact push | `ommx.artifact.push` | active span |

Trace ID は OTel が発行する。OMMX は独自 Trace ID を採番しない。

### 6.2 Record / parameter event

各 `log_*` は Record 追加または Run parameter table 更新の後、可能なら current span に event を追加する。

Event 名:

- `ommx.record.added`
- `ommx.run.parameter.recorded`

Record event attributes:

| Attribute | 内容 |
|---|---|
| `ommx.record.space` | `experiment` / `run` |
| `ommx.record.run_id` | run space の場合 |
| `ommx.record.kind` | `solution`, `instance`, `diagnostic`, ... |
| `ommx.record.name` | Record name |
| `ommx.record.media_type` | payload media type |
| `ommx.record.digest` | commit 後に分かる場合。Build 中は absent でもよい |

Run parameter event attributes:

| Attribute | 内容 |
|---|---|
| `ommx.run.id` | run id |
| `ommx.run.parameter.name` | parameter name |
| `ommx.run.parameter.scalar_type` | `int`, `float`, `string`, `bool`, `null` 等 |

Event は Record または Run parameter cell への reference であり、payload 本体を OTel event attribute に入れない。parameter の small scalar を display 用に入れるかは renderer policy として扱い、正本にはしない。

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

Trace layer は Record / Run parameter table / Run attributes の代替ではない。parameter / solution / sample set / environment の本体は Experiment state の物理化戦略に従って保存し、trace layer は実行時系列と record reference を保存する。

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
| manifest annotation | `org.ommx.artifact.kind=experiment` |
| manifest annotation | `org.ommx.experiment.schema=v1` |
| optional manifest annotation | `org.ommx.experiment.status=finished|failed` |
| optional manifest annotation | `org.ommx.experiment.recovery=true` |
| optional index layer media type | `application/org.ommx.v1.experiment+json` |

`Artifact.load()` は従来通り OMMX Artifact として読み、`Experiment.load()` は manifest annotation、Experiment metadata / index layer、Record layer annotations を見て Experiment view を復元する。これにより、Experiment は OMMX Artifact family の一種として扱え、既存の Local Registry / archive / remote transport / generic Artifact inspector と互換にできる。

通常の成功 commit は `status=finished` 相当の Experiment Artifact として requested tag / ref に publish する。例外終了時に作る failed recovery manifest は `status=failed` と `recovery=true` 相当の annotation を持ち、`<registry-id8>.ommx.local/crashed:<local-timestamp>-<nonce>` のような reserved ref に publish する。`Experiment.load(tag)` の通常 UX は requested tag / ref の成功 Artifact を読む。recovery manifest は recovery command / inspector から明示的に扱う。

`OMMX Artifact v3` という media type は導入しない。v3 は SDK / 設計フェーズの名前であり、wire format の互換性境界とは分ける。将来、registry の referrers API などで Experiment だけを `artifactType` で filter したい要求が強くなった場合は、`application/org.ommx.v1.experiment` を追加で許容する余地を残す。ただし初期設計では、top-level は `application/org.ommx.v1.artifact` に統一する。

### 7.2 完全な descriptor set としての manifest

各 committed Artifact manifest は、blob bytes の保存タイミングを表すものではない。`layers[]` には、その時点の Experiment view を復元するために必要な typed payload、aggregate JSON、index JSON などの descriptor を載せる。

Run は manifest の子 manifest ではなく、Experiment manifest 内の layer 群から復元する。初期設計では、少なくとも以下の aggregate payload を通常の Artifact layer として載せる。

| Layer | 目的 | 備考 |
|---|---|---|
| Experiment index JSON | run list、Record descriptor index、table reconstruction hints | `application/org.ommx.v1.experiment+json` など |
| Run parameter table JSON | run ごとの scalar parameter table | 1 cell = 1 layer にはしない |
| Run attributes JSON | run status、elapsed time、実行環境属性など | 実行環境は Record ではない |

これらは manifest annotation だけで表現しない。Manifest annotations は Artifact kind / schema / small metadata を表すために使い、Run table や Run attributes の本体は layer payload として保存する。

Run-scoped Record は個別 layer または aggregate layer として保存し、descriptor annotations に `org.ommx.experiment.space=run` と `org.ommx.experiment.run_id=<id>` を持たせる。`Experiment.load(...)` は aggregate payload と Record layer annotations を読んで `run_id` ごとに group 化し、Run view を復元する。

Instance / Solution / diagnostics などの payload blob は `log_*` 時点で Local Registry の BlobStore に逐次保存される。commit が行うのは、それらの blob と、Run parameter table / Run attributes JSON など commit 時に materialize する payload を含む最終 Experiment state を seal し、復元に必要な descriptor set を immutable manifest として IndexStore に publish することである。

既存 blob は同じ digest の descriptor として再利用できる。Local Registry では CAS として共有され、remote registry では dedup / mount され得る。一方、archive export では、その Artifact 単体で読めるよう参照 blob を含める。

したがって、複数の Run が同一 bytes の Instance を保存する場合、論理的な run Record / descriptor は複数存在してよいが、BlobStore 上の実体は同じ digest の 1 blob に共有される。これは API を「run ごとの Record」として見せる設計とも、「Record が blob descriptor を直接管理する」設計とも両立する。重複排除の前提は serialized bytes が一致することであり、論理的に同じ Instance でも serialization に timestamp や非決定的 ordering が混ざる場合は別 digest になる。

### 7.3 Layer annotations

Record layer は Artifact layer descriptor annotations に以下を持つ。

| Annotation | 必須 | 内容 |
|---|---|---|
| `org.ommx.experiment.space` | yes | `experiment` / `run` |
| `org.ommx.experiment.run_id` | run only | decimal run id |
| `org.ommx.record.kind` | yes | Record kind |
| `org.ommx.record.name` | yes | Record name |
| `org.ommx.record.scalar_type` | scalar record only | `int`, `float`, `string`, `bool`, `null` 等 |
| `org.ommx.codec` | optional | external codec identifier, 必要な場合 |

Run parameter table と Run attributes は、必ずしも 1 cell / attribute = 1 layer descriptor にならない。run parameter の key-level metadata や実行環境属性は aggregate payload の内部 schema に持たせてよい。Experiment metadata を manifest annotation に物理化する場合も、`parameter` ではなく `metadata` として復元する。

`media` Record の descriptor `mediaType` は caller / external package が指定した値をそのまま使う。OMMX core は unknown media type を拒否せず、`org.ommx.record.kind=media`、`org.ommx.record.name`、必要なら `org.ommx.codec` を保持して opaque bytes として扱う。

Experiment name、created time、OMMX version などの experiment-level metadata は manifest annotations または dedicated metadata Record に保存する。巨大な metadata は manifest annotation に載せず Record にする。

MINTO 由来の `org.minto.*` annotation は新規書き込みでは使わない。既存 MINTO artifact の import compatibility が必要なら、compat loader が `org.minto.*` を読んで `org.ommx.*` Record model に変換する。

### 7.4 Artifact からの復元

`Experiment.load(...)` は Artifact を読み、layer annotations から immutable Experiment view を復元する。

復元に必要な invariants:

- `org.ommx.experiment.space` がない OMMX layer は Experiment Record ではないため無視してよい。
- run id は 0-based integer とし、欠番がある場合は empty run view を作るか、strict mode で error にする。
- 同一 `(space, run_id, kind, name)` の Record が複数 layer に現れた場合、committed manifest では重複を禁止し、loader は error にする。
- Run parameter table で同一 `(run_id, parameter_name)` が複数値を持つ場合も、committed manifest では重複を禁止し、loader は error にする。
- Run attributes で同一 `(run_id, attribute_name)` が複数値を持つ場合も、committed manifest では重複を禁止し、loader は error にする。

## 8. Lineage

Artifact lineage は OCI v1.1 `subject` で表す。初期設計では各 Artifact が 0/1 個の parent を持つ single-parent history のみを扱う。複数 child は自然に発生してよい。

| API | 方針 |
|---|---|
| `parent()` | `subject` を読む。0/1 件 |
| `history()` | `subject` chain を root 方向に辿る |
| `diff(other)` | Record set、Run parameter table、Run attributes、layer descriptor を比較する |

`subject` は provenance / lineage 用リンクであり、Artifact 復元に必須の dependency ではない。各 manifest は復元に必要な descriptor set を持つので、単一 Artifact archive は parent chain なしで読める。

保存済み Experiment に run を追加する場合は、loaded `Experiment` をその場で mutable にせず、`with exp.fork(tag=...) as forked:` のように parent Artifact から派生した新しい session / builder を作る。re-enter はこの派生 session を開く操作であり、元 Artifact を変更する操作ではない。正常終了時の自動 commit では新しい manifest を作り、`subject` に parent manifest descriptor を記録する。既存 run の descriptor / blob は再利用し、新しい run で追加された Instance / Solution / diagnostics / aggregate JSON だけが BlobStore に追加される。

同じ仕組みで、派生した Experiment version から run を削除する操作も表現できる。削除は既存 blob を消す操作ではなく、新しい manifest からその run に対応する descriptor、Run parameter table row、index record を省く操作である。元 Artifact は immutable な parent として残り、削除された run の blob は parent からは引き続き参照され得る。物理的な blob 削除は GC の到達可能性解析と retention policy に委ねる。

複数 experiment の統合は lineage merge としては扱わない。必要なら新規 Artifact の Record として入力 Artifact digest を列挙する。これは parent ではなく data reference である。

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

GC は data model を変えない。完全な descriptor set を持つ manifest、digest primary、single-parent lineage、Record model / Run parameter table / Run attributes とは独立した maintenance operation とする。

## 10. 未決定事項

実装前に決める必要がある点:

1. `Experiment` の Python module path と API 名
   `ommx.experiment.Experiment` とするか、`ommx.artifact.Experiment` 配下に置くか。

2. Context manager の詳細 semantics
   `with Experiment(...)` は正常終了時に自動 commit する方針とする。残る論点は、commit 済み session への追加を禁止する API、`exp.artifact` の availability、例外終了時の failed recovery manifest UX、`crashed:<timestamp>-<nonce>` ref の表示 / pruning、tag が省略された場合の扱いである。

3. Fork semantics
   `exp.fork(tag=...)` の tag 解釈、parent metadata の継承 / 上書き、forked session の名前、同一 parent から複数 child が作られる場合の UX を決める。

4. Autosave metadata
   Blob 本体は Local Registry BlobStore に逐次保存する。未 commit の Experiment state / recovery metadata をどこに置くか、failed recovery manifest にどこまで含めるか、どの粒度で復元可能にするかを決める。

5. Record / Run table / Run attributes の物理化境界
   大きな Record payload は `log_*` 時点で BlobStore に書く前提で、どの kind を即 descriptor 化するか。Run parameter table と Run attributes をどの aggregate JSON / index JSON に materialize するか。

6. Duplicate Record / parameter handling
   Build phase の upsert は許容するが、Seal 時にどの単位で正規化して 1 値にするか。

7. Run deletion と lineage retention
   派生 Experiment version で run を省く API をどの範囲で提供するか。また、削除された run の blob を物理的に reclaim するための parent lineage pruning / retention policy をどう設計するか。

8. Adapter diagnostics protocol
   `diagnostics` kwarg を signature inspection で渡すか、`SupportsDiagnostics` marker / class attribute で検出するか。`DiagnosticEntry` / `DiagnosticsSink` / `DiagnosticCollector` の Python / Rust 型定義と media type validation を決める。

9. Table extraction の責務
   summary extraction を Rust core に持つか、Python-only view に寄せるか。

10. Trace provider setup UX
    Experiment core は global provider を install しない方針で固定する。一方、notebook helper / magic が UX のため provider を install することを許すか。

11. Legacy MINTO artifact import
    `org.minto.*` annotation を読む compatibility loader を OMMX に持つか、別 migration tool にするか。

## 11. 実装トラック

### Track A: Experiment / Run / Record model の中核

- `Experiment`, `Run`, immutable loaded `Experiment` view, forked session を設計する。
- Record model、Run parameter table、Run attributes、typed storage API を実装する。
- Run 実行環境属性を Run attribute として実装する。
- Build phase upsert と Seal phase normalization を実装する。
- `with Experiment(...)` の正常終了 auto commit、例外終了時の failed recovery manifest、commit 後 mutation 禁止を実装する。
- Python tests で MINTO の主要 UX を再現する。

### Track B: Artifact への写像と table view

- Record、Run parameter table、Run attributes を Artifact layer / aggregate JSON に materialize する annotation schema を実装する。
- Artifact から Experiment view を復元する loader を実装する。
- `get_run_table()` / experiment-level table view を実装する。
- `org.minto.*` compatibility の扱いを決め、必要なら import path を実装する。

### Track C: Adapter execution / diagnostics

- `run.log_solve(...)` / `run.log_sample(...)` を adapter protocol に沿って実装する。
- scalar kwargs を Run parameter table に記録する。
- adapter diagnostics optional protocol を設計する。
- diagnostics Record と OTel summary event の連携を実装する。

### Track D: OTel trace integration

- Experiment / Run / solver / build / load / push span schema を実装する。
- `log_*` record / parameter event を実装する。
- `trace="auto" | "required" | False` を実装する。
- global `TracerProvider` を暗黙に設定しないことを tests で固定する。

### Track E: Trace layer と renderer

- OTLP JSON trace layer を Artifact に埋め込む。
- `artifact.get_trace() -> TraceResult` を実装する。
- post-hoc text tree renderer を Experiment style に対応させる。
- Chrome Trace Event Format export を derived view として提供する。

### Track F: Lineage / fork API

- `subject` を使った `parent()` / `history()` を実装する。
- `exp.fork(tag=...)` を実装する。
- Record set、Run parameter table、Run attributes に基づく `diff(other)` を実装する。
- 各 Artifact が 0/1 parent を持つ single-parent history の制約を tests で固定する。

### Track G: GC

- reachability analysis hook を実装する。
- Local Registry GC の dry-run / report / delete flow を実装する。
- archive / OCI directory GC の候補列挙を実装する。
- remote registry は capability detection と dry-run を優先する。

## 12. 第1イテレーション実装計画

Track A の中核のうち、最小の happy path を最初に通す。Local Registry / `LocalArtifactBuilder` は Rust 専用機能なので、まず `ommx` crate にコア `experiment` モジュールだけを実装する。Python SDK への PyO3 expose は本イテレーションでは扱わない。

### 12.1 スコープ

到達点: `Experiment` 開始 → `Run` 開始 → `Record` 追加 → `Run` 終了 → `Experiment` 終了で 1 Artifact を commit する Rust core API。

| 含む | 含まない（後続イテレーション） |
|---|---|
| `Experiment` / `Run` の Build phase session と lifecycle API（`run()` / `finish()` / `fail()` / `commit()`） | Python SDK への PyO3 expose（`ommx.experiment` / context manager） |
| Record（`metadata` / `object` / `instance` / `solution` / `sampleset`）の experiment space / run space への追加 | 失敗時処理全般（3.4 の crash recovery manifest / orphan blob / autosave metadata） |
| Run lifecycle（status / elapsed time の Run attributes 反映） | Run parameter table（`log_parameter` / `get_run_table`） |
| `log_*` 時点の BlobStore 逐次書き込みと `commit()` の manifest publish（新規 `local_registry` primitive を含む） | `Experiment.load()` による immutable view 復元 |
| Build phase の `(space, run_id, kind, name)` upsert、commit 後 mutation 禁止 | OTel / trace layer / renderer、fork / lineage / `diff` |
| | `log_solve` / `log_sample`、diagnostics sink、`media` Record、実行環境属性、GC |

第1イテレーションから逐次書き込みを採用する。`log_*` は payload をその場で Local Registry の BlobStore に CAS blob として書き、in-memory には descriptor だけを残す。`commit()` は BlobStore に書き込み済みの blob 群を参照する manifest を組んで IndexStore に publish する。bytes を commit まで in-memory に貯める設計は、最終形（逐次書き込み）で捨てる throwaway になるため採らない。本イテレーションで外すのは crash recovery の機構（recovery manifest / autosave metadata / orphan blob 回収）であって、逐次書き込みという機構自体は最初から入れる。crash した場合は orphan blob が残るが、その回収は GC イテレーションに委ねる。

### 12.2 Rust core: `ommx::experiment`

`rust/ommx/src/experiment.rs` を新設し、`lib.rs` に `pub mod experiment;` を追加する。`Experiment` は構築時に `Arc<LocalRegistry>` を保持し、`log_*` のたびに BlobStore へ書き込む。

| 型 | 役割 |
|---|---|
| `Experiment` | `Arc<LocalRegistry>` + `Arc<Mutex<ExperimentState>>`。Build phase の mutable session |
| `Run` | `Arc<LocalRegistry>` + `Arc<Mutex<ExperimentState>>` + `run_id`。Experiment state を共有 |
| `ExperimentState`（private） | name, requested ref, experiment space records, runs, next run id, committed flag, commit 済み `LocalArtifact` |
| `RunState`（private） | run_id, run space records, status, started_at, elapsed |
| `Record`（private） | space, run_id, kind, name, BlobStore に書き込み済み blob を指す descriptor + `BlobRecord` |
| `Space` / `RecordKind` / `RunStatus` enum | `experiment`/`run`、`metadata`/`object`/`instance`/`solution`/`sampleset`、`running`/`finished`/`failed` |

API:

- `Experiment::new(name)`（default Local Registry を開く） / `Experiment::with_registry(name, Arc<LocalRegistry>, Option<ImageRef>)`（テスト用に registry / requested ref を差し替え可能）
- `Experiment::log_metadata` / `log_object` / `log_instance` / `log_solution` / `log_sample_set`（experiment space）
- `Experiment::run() -> Run`（`run_id` を 0-based で採番）
- `Experiment::commit() -> LocalArtifact`
- `Experiment::artifact() -> LocalArtifact`（commit 後のみ available、commit 前は error）
- `Experiment::is_committed() -> bool`
- `Run::run_id()`、`Run::log_metadata` / `log_object` / `log_instance` / `log_solution` / `log_sample_set`（run space）、`Run::finish()` / `Run::fail()`

挙動:

- `log_*` は payload を即 `FileBlobStore::put_bytes` で BlobStore に書き、戻り値から組んだ descriptor / `BlobRecord` を `Record` に保持する。payload bytes は in-memory に残さない。
- `log_*` は同じ `(space, run_id, kind, name)` を upsert（replace）する。upsert で捨てられた古い blob は orphan blob になり GC に委ねる。
- commit 済み Experiment への `log_*` / `run()` は error。
- `commit()` は idempotent。2 回目は既存 `LocalArtifact` を返す。
- error は `crate::bail!` / `crate::error!` fail-site macro を使う。

### 12.3 Artifact への写像と新規 primitive

`log_*` は `LocalArtifactBuilder` を使わず、`registry.blobs().put_bytes(bytes)` で直接 BlobStore に CAS blob を書き、experiment / record annotations を載せた descriptor を組み立てる。

`commit()` は次を行う:

1. Run attributes JSON / Experiment index JSON / OCI empty config を `put_bytes` で BlobStore に書く。
2. 全 Record descriptor + aggregate layer descriptor + empty config から `ImageManifest`（`artifactType` = `application/org.ommx.v1.artifact`、manifest annotations 付き）を組み、stable JSON bytes と manifest descriptor を計算する。
3. tag 未指定なら `registry.synthesize_anonymous_image_name()` で anonymous image name を採番する。
4. 新規 primitive で IndexStore に atomic publish する。

新規 primitive は既存 `publish_artifact_manifest` の sibling とする。`publish_artifact_manifest` は `&[StagedArtifactBlob]`（descriptor + bytes）を受け取り blob ごとに `put_bytes` するが、新 primitive は BlobStore 書き込み済み blob の `BlobRecord` を受け取り bytes を再供給しない。IndexStore への atomic 書き込みは既存の `SqliteIndexStore::publish_artifact_atomic`（既に `pub`・bytes 非依存）に乗る。`publish_artifact_manifest` から validation / record 組み立て / `publish_artifact_atomic` 呼び出しの共通部分を切り出して両者で共有する。

byte-level で同一の payload は CAS で 1 物理 blob に共有される（§7.2）。論理 Record は `(space, run_id, kind, name)` 単位なので、複数 Run が同じ `name`・同じ bytes の Record を持つ場合でも `run_id` が異なれば別 Record として両立し、同じ digest を annotations 違い（`org.ommx.experiment.run_id` 等）の複数 descriptor が指す。したがって `commit()` は manifest `layers[]` には Record ごとの全 descriptor を載せる一方、新規 primitive へ渡す `BlobRecord` 列は digest で de-dup した unique 集合にする。新規 primitive の「manifest layer ↔ staged blob」検証は、`BlobRecord` が annotations を持たず同一 digest の layer descriptor が複数並び得るため、既存 `publish_artifact_manifest` の full descriptor 一致ではなく digest 単位で行う。

manifest annotations:

| Key | Value |
|---|---|
| `org.ommx.artifact.kind` | `experiment` |
| `org.ommx.experiment.schema` | `v1` |
| `org.ommx.experiment.name` | experiment name |
| `org.ommx.experiment.status` | `finished` |

Record layer（`log_*` 時に descriptor へ付与）:

| Annotation | 内容 |
|---|---|
| `org.ommx.experiment.space` | `experiment` / `run` |
| `org.ommx.experiment.run_id` | run space のみ |
| `org.ommx.record.kind` | `metadata` / `object` / `instance` / `solution` / `sampleset` |
| `org.ommx.record.name` | Record name |

Record media type: `instance` / `solution` / `sampleset` は `application/org.ommx.v1.*`、`metadata` / `object` は `application/json`。

aggregate layer（`org.ommx.experiment.space` を持たないので loader は Record として扱わない）:

| Layer | media type | layer annotation |
|---|---|---|
| Run attributes JSON | `application/org.ommx.v1.experiment.run-attributes+json` | `org.ommx.experiment.layer=run-attributes` |
| Experiment index JSON | `application/org.ommx.v1.experiment+json` | `org.ommx.experiment.layer=index` |

OCI manifest descriptor の media type と `artifactType` は既存 Artifact 仕様のまま（`application/vnd.oci.image.manifest.v1+json` / `application/org.ommx.v1.artifact`）。

### 12.4 テスト

`experiment.rs` の `#[cfg(test)]` で次を検証する。

- run lifecycle（`run()` での `run_id` 採番、`finish()` / `fail()` による status / elapsed 反映）
- `log_*` 時点で payload が temp registry（`with_registry`）の BlobStore に書かれること、同一 `(space, run_id, kind, name)` の upsert
- `commit()` 後の manifest annotations / Record layer annotations / aggregate layer
- commit 後の `log_*` / `run()` が error になること、`commit()` の idempotency

### 12.5 後続イテレーション

Python SDK への PyO3 expose（`ommx.experiment` の `Experiment` / `Run` / context manager）、失敗時処理（3.4 の BlobStore 逐次 autosave / recovery manifest / autosave metadata）、Run parameter table、`Experiment.load()`、trace、fork / lineage、`log_solve` / `log_sample` と diagnostics、GC は本計画のあとに追加する。
