# OMMX Experiment / Artifact v3 提案

OMMX v3 における Experiment / Run / Record / Trace / Lineage / GC の未実装領域をまとめる提案。

本ファイルは開発中の一時文書である。実装完了後は削除し、内容を通常の Sphinx documentation / API reference / module rustdoc に統合する。既に実装済みの Artifact manifest format、Local Registry、archive / remote transport の移行ログは本書では扱わない。必要な前提は `rust/ommx/doc/artifact_design.md` と `ommx::artifact::local_registry` の rustdoc を参照する。

本書の主眼は、MINTO が提供していた実験管理 UX を OMMX-owned な機能として再設計し、記録データ、実行 telemetry、Artifact version、lineage を一貫したモデルに落とすことである。

## 1. 目標 UX

OMMX が提供する実験管理機構の最終 UX は、まず以下の形を目標にする。API 名は提案であり、実装時に Python / Rust の慣習に合わせて調整してよい。

```python
from ommx.experiment import Experiment

with Experiment("scip_reblock115", trace="auto", autosave=True) as exp:
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

artifact = exp.commit(tag="scip_reblock115:latest")

table = exp.get_run_table()
trace = artifact.get_trace()
```

この UX で保証したいこと:

- experiment 全体のデータと run 固有のデータを明確に分離できる。
- dataset / source problem / baseline configuration は experiment space に metadata / object として記録できる。
- run ごとに変わる formulation / solver kwargs などの parameter を Run table の列として記録し、後から比較できる。
- Instance、solution、sample set、object、diagnostics は Experiment / Run のどちらにも Record として記録できる。
- parameter は他の Record kind とは区別し、Experiment が持つ Run 一覧に付随する表形式データとして扱う。
- `log_*` は logger API ではなく、Experiment state への記録 API である。
- console 表示は記録の副作用ではなく、trace renderer が作る view である。
- `commit()` は immutable Artifact version を作る。
- Artifact は digest primary、tag は mutable alias として共有に使う。
- Artifact を archive / Local Registry / remote registry から reload して、table と trace を再構成できる。
- 実験途中で process が落ちても、`autosave=True` なら既に記録したデータを可能な範囲で復元できる。

## 2. MINTO から維持する体験

MINTO 互換 API は維持しない。ただし、以下のユーザー体験は維持対象とする。

### 2.1 2 つの保存空間

MINTO の中心 UX は、実験全体に属するデータと run ごとに属するデータを分ける 2 つの保存空間モデルである。

このモデルは MLflow の Experiment / Run 階層と似ているが、OMMX では Experiment を単なる run のグループにはしない。数理最適化の実験では、同じ source problem から定式化を変える、一部の制約を無効化する、relaxation を変える、solver parameters を変える、といった複数 run を比較することが多い。したがって run 固有の Instance は自然に Run space に記録される。Experiment space は全 run に共通する source data、baseline、dataset identity、analysis context を持ち、共有入力、複数試行、比較 table、共有 Artifact を 1 つの experiment bundle として扱う。

| 保存空間 | 目的 | 例 |
|---|---|---|
| Experiment space | 全 run で共有される context | dataset name, source problem, baseline config |
| Run space | 各 run の定式化、条件、実行環境、結果 | run parameters, candidate instance, disabled constraints, environment, solution, sample set |

OMMX では `log_global_*` という命名は採用しない。Experiment object に対する `exp.log_*` は experiment space に、Run object に対する `run.log_*` は run space に記録する。保存先は receiver で決まり、暗黙の context では決めない。ただし `parameter` は Record kind ではなく Run table の列データであり、Experiment には `exp.log_parameter` を持たせない。

Record kind は基本的に space に依存しない。`metadata`, `object`, `instance`, `solution`, `sampleset`, `diagnostic`, `media` は Experiment space / Run space のどちらにも置ける。例えば全 run で共有する source Instance は Experiment space に、実際に各 run で解いた candidate Instance は Run space に置く。

例外として `environment` は Run scope 専用の Record とする。保存済み Experiment から後で新しい Run を追加できる以上、実行環境は Experiment 全体の不変属性ではない。Experiment を作成した環境や import / load した環境を残したい場合は、`environment` ではなく metadata / provenance として扱う。

```python
exp.log_metadata("dataset", "miplib2017")      # experiment space

with exp.run() as run:
    run.log_instance("candidate", instance)    # run space
    run.log_parameter("seed", 0)               # run table parameter
    run.log_solution("result", solution)       # run space
```

### 2.2 分析用テーブル

`get_run_table()` 相当の table view は維持する。これは Artifact に保存された Record set / Run parameter table から作る分析用の派生 view である。

- Run parameter はそのまま column 化する。
- `Solution` は objective / feasible / optimality / relaxation / start などの summary を column 化する。
- `SampleSet` は sample count / objective mean, std, min, max / feasible count などの summary を column 化する。
- JSON object や大きな binary payload は default table には展開しない。必要なら user が `exp.runs[run_id].objects[...]` や `artifact.get_*` から取り出す。

Python SDK では pandas `DataFrame` を返す。Rust SDK では table data を構造化表現として提供し、Python 側で DataFrame に変換してよい。

### 2.3 Adapter 実行

OMMX では Adapter API が `SolverAdapter.solve(...)` / `SamplerAdapter.sample(...)` として標準化されているため、主要 UX は `run.log_solve(...)` / `run.log_sample(...)` にする。任意 callable を包む generic solver logging API は提供しない。

目標:

- `log_solve` は `SolverAdapter.solve(...)` を呼び、`Solution` を run space に記録する。
- `log_sample` は `SamplerAdapter.sample(...)` を呼び、`SampleSet` を run space に記録する。
- adapter name / backend solver name を run metadata として記録する。
- scalar kwargs を Run table の parameters として記録する。
- `Instance` kwargs は、その run で実際に解いた candidate Instance として記録できる。全 run で本当に共有される巨大な Instance だけは、experiment space Record への明示的な reference を使えるようにする。
- 実行時間は OTel span に記録し、Artifact に保存する記録データの正本にはしない。
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

### 2.4 Adapter 診断情報

ソルバーの native log / report は、構造化された結果の正本ではない。しかし、モデルが解けない、timeout する、infeasible になる、または想定外の挙動をするときの解析には不可欠な診断 evidence である。OMMX はこの evidence を Artifact に保持できる必要がある。

どれだけの診断情報を保存すべきかはソルバー依存である。OMMX core が stdout / stderr を盲目的に capture し、すべての backend に対する policy を決めるべきではない。代わりに、各 OMMX Adapter が対象 backend solver の診断 policy を持つ。

Adapter 側の責務:

- debug に有用な native log / report / summary を選ぶ。
- presolve log、node log、gap history、termination report、warning、backend status、backend version、sampler schedule など、ソルバー固有の evidence を capture する。
- truncate、compression、redaction、summary の policy を決める。
- optional adapter protocol を通じて diagnostics を公開する。

OMMX core 側の責務:

- optional diagnostics protocol が利用可能なら呼び出す。
- diagnostics を `media` / `object` / `diagnostic` Record として保存する。
- diagnostics Record を committed Artifact に含める。
- diagnostics Record を要約し参照する OTel span attribute / event を出す。

したがって diagnostics は 2 層構造にする。

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

### 2.5 共有

MINTO の archive / registry sharing UX は維持する。ただし `Experiment` は最終的に OMMX Artifact を build / commit する front-end であり、共有の正本は Artifact である。

```python
artifact = exp.commit(tag="ghcr.io/org/repo/scip_reblock115:v1")
artifact.push()

loaded = Experiment.load("ghcr.io/org/repo/scip_reblock115:v1")
loaded.get_run_table()
```

`save_as_ommx_archive()` / `load_from_ommx_archive()` のような名前は compatibility layer として置いてもよいが、中心概念は `commit()` と `Artifact.load*` に寄せる。

## 3. 責務分担

Experiment v3 は、記録データ、実行 telemetry、表示、Artifact version を明確に分ける。

| 領域 | 正本 | 例 |
|---|---|---|
| 記録データ | Artifact manifest / layers / aggregate payload | run parameters, metadata, objects, Instance, Solution, SampleSet, EnvironmentInfo |
| 実行 telemetry | OTel trace | lifecycle, duration, solver execution, IO, error, record event |
| console / notebook 表示 | Trace renderer | text tree, live view, Chrome trace export |
| version / sharing | Artifact manifest | digest, tag, subject lineage, layer descriptors |

`run.log_parameter(...)` の一次効果は Run table の parameter cell 更新である。`run.log_solution(...)` や `run.log_instance(...)` の一次効果は Record の追加である。同時に OTel span event を出すことはできるが、それは「この run で何が記録されたか」を可視化する telemetry であり、データ本体ではない。

## 4. Experiment Record モデル

### 4.1 Record と Run parameter table

Experiment state は名前付き Record set と Run parameter table からなる。ここで Record と呼ぶものは API / loader から見える単位であり、実装上の blob 所有単位を意味しない。

物理化戦略は kind ごとに変えてよい。`log_instance` / `log_solution` / `log_sample` / 大きな diagnostics payload は `log_*` 時点で BlobStore に保存し、Record が digest / size / media type / annotations を持つ descriptor reference になる設計が自然である。一方、`run.log_parameter` のような小さな scalar は Record にはせず、Experiment が持つ Run 一覧に付随した表形式データとして扱う。Experiment scope の scalar context は `parameter` ではなく `metadata` として扱い、必要なら manifest annotation または Experiment metadata JSON に物理化する。この節の field は論理モデルであり、最終的な Rust struct の field を固定するものではない。

各 Record は概念的に space、kind、name、content、metadata を持つ。Record kind は原則として space によって制限しない。ただし `environment` は Run scope 専用である。`parameter` は Record ではないため、この規則の対象外である。

| Field | 内容 |
|---|---|
| `space` | `experiment` または `run` |
| `run_id` | run space の場合のみ必須 |
| `kind` | `metadata`, `object`, `instance`, `solution`, `sampleset`, `environment`, `diagnostic`, `media` |
| `name` | space + kind 内の user-facing key |
| `content` | scalar value、serialized bytes、または blob descriptor への参照 |
| `media_type` | Artifact layer media type。単独 layer にならない Record では aggregate payload 内の type 情報でもよい |
| `annotations` | Artifact descriptor annotations に投影される metadata |

Run parameter table は Record とは別に、`run_id` と parameter name を key にした scalar table として持つ。

Build phase では同じ `(space, run_id, kind, name)` に対する Record upsert と、同じ `(run_id, parameter_name)` に対する parameter upsert を許容する。Seal / commit phase では最終 Experiment state を復元できる aggregate payload と descriptor set に固定する。View phase の `Artifact` / loaded `Experiment` は immutable とし、追加や更新は新しい builder / Experiment から別 Artifact を作る。

### 4.2 Run parameter table の物理化

MINTO は `parameters` / `meta_data` を aggregate JSON dict として保存していた。OMMX v3 では、`parameter` を Record ではなく Run table に付随する列データとして扱う。Experiment 全体に属する dataset、source problem、baseline、analysis context は `metadata` または `object` であり、`parameter` ではない。

Run parameter を key ごとの独立 blob にする必要はない。Experiment state の論理 view では `run.log_parameter("seed", 0)` を Run table の cell update として扱い、table extraction や diff は key 単位でできるようにする。一方、Artifact への物理化では小さな scalar をまとめる。

例:

```python
run.log_parameter("timelimit", 1.0)
run.log_parameter("seed", 0)
```

この 2 つは論理的には別 parameter cell である。ただし物理的には、例えば run ごとの parameter aggregate JSON または Experiment index JSON にまとめて保存してよい。

物理化候補:

| 対象 | 候補 | 備考 |
|---|---|---|
| run parameter | Run ごとの parameter aggregate JSON または Experiment index JSON | `get_run_table()` の入力になる |
| adapter kwargs | run parameter として aggregate JSON に含める | scalar kwargs のみ。Instance などは別 Record |
| table index | Experiment index JSON | table 再構成を速くするための derived payload として持てる |

この方針では、`parameter` は API / analysis 上は key 単位で扱えるが、Record ではなく Artifact layer の最小単位でもない。`Instance` や `Solution` のような大きな typed payload は Record として即 blob 化しやすいが、scalar parameter は commit 時に Run table payload として materialize する方が manifest / blob 数を抑えられる。

### 4.3 対応する Record 種別

Core が直接扱う Record:

| Kind | Payload | 備考 |
|---|---|---|
| `metadata` | JSON | dataset name / source problem id / system metadata などの小さな context。構造化 config は `object` を優先 |
| `object` | JSON | JSON serializable dict / list 等 |
| `instance` | `ommx.v1.Instance` bytes | public API は `ommx.v1` |
| `solution` | `ommx.v1.Solution` bytes | table summary を持つ |
| `sampleset` | `ommx.v1.SampleSet` bytes | table summary を持つ |
| `environment` | `EnvironmentInfo` JSON | run scope only。OTel Resource への投影元 |
| `diagnostic` | JSON または bytes | solver / adapter diagnostic evidence |
| `media` | 任意の bytes | external package 用 |

OMMX core は `jijmodeling` を import しない。domain-specific problem storage は external package が `media_type` と codec を登録して提供する。

### 4.4 EnvironmentInfo

`EnvironmentInfo` は Run scope の first-class Record として保存する。OTel `Resource` はその投影であり、情報本体ではない。

Run は後から追加実行できるため、実行環境は Experiment 全体に固定できない。`EnvironmentInfo` は各 Run の実行環境を表す。Experiment 作成時の SDK / host 情報を残す必要がある場合は、実行環境ではなく provenance metadata として保存する。

保存対象:

- OS / platform
- host / CPU / memory
- process / Python / Rust runtime
- package versions
- container / CI metadata, 取れる場合
- OMMX / adapter version

OTel Resource へ写す属性は standard semantic conventions を優先する。標準属性で表現できない OMMX 固有情報だけを `ommx.*` namespace に置く。同じ意味の値を標準属性と `ommx.*` に二重記録しない。

## 5. Experiment ライフサイクル

### 5.1 Build / Seal / View の 3 相

Experiment / Artifact の変更可能性は 3 相に分ける。

| 相 | 性質 | API |
|---|---|---|
| Build | メモリ上では mutable。autosave で durable draft を持てる | `Experiment`, `Run`, `ArtifactBuilder` |
| Seal | immutable Artifact version を作る | `commit()` / `build()` |
| View | read-only | `Artifact`, loaded `Experiment`, table / trace view |

永続化済み Artifact を更新する API は作らない。既存 Artifact から派生して新しい Artifact version を作る場合は、parent を lineage として記録する。

### 5.2 Commit 粒度

デフォルトは `1 Experiment = 1 manifest` とする。

- `Experiment.commit()` は実験全体の immutable Artifact version を作る。
- Local Registry に public Artifact manifest / tag / ref として現れるのは `commit()` 時点である。
- `with Experiment(...)` は正常終了時に自動 commit してよいが、明示 `commit()` との関係を API で明確にする。
- `Run` 終了ごとに manifest を切る挙動は `commit_per_run=True` 相当の opt-in にする。
- デフォルトでは run ごとに commit しない。

理由:

- MINTO の UX は「複数 run を 1 experiment として比較する」ことが中心である。
- run ごとに manifest を作ると lineage が細かくなりすぎ、table reconstruction と sharing が複雑になる。
- 一方、長時間実験では途中結果を durable にしたいので autosave / draft storage は別に必要である。

### 5.3 Autosave

`autosave=True` は「各 `log_*` のたびに最終 Artifact を commit する」ではない。

目標:

- process crash 後に記録済み entries を復元できる。
- final `commit()` までは public tag / digest を進めない。
- autosave storage は Local Registry の draft area または SDK-owned working directory として扱う。
- autosave draft から final Artifact を作るときも、manifest は復元に必要な descriptor を完全に列挙する。

`log_*` 時点で大きな Instance / solver log / diagnostics payload を BlobStore や draft area に spill してよいが、それは未 publish の draft data である。public ref に到達可能な Artifact manifest は `commit()` まで作らない。

autosave の storage format は user-facing compatibility surface にしない。directory layout compatibility より、復元可能性と final Artifact semantics を優先する。commit されずに残った draft metadata や、draft / spill のために作られた未到達 blob は GC の対象になる。

## 6. OTel / Trace モデル

### 6.1 Span 階層

OMMX は global `TracerProvider` を暗黙に設定しない。Experiment / Run / ArtifactBuilder は active provider がある場合にそれを使い、ない場合は trace capture mode に従う。

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

Trace layer は Record / Run parameter table の代替ではない。parameter / solution / sample set / environment の本体は Experiment state の物理化戦略に従って保存し、trace layer は実行時系列と record reference を保存する。

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
| optional index layer media type | `application/org.ommx.v1.experiment+json` |

`Artifact.load()` は従来通り OMMX Artifact として読み、`Experiment.load()` は manifest annotation、Experiment metadata / index layer、Record layer annotations を見て Experiment view を復元する。これにより、Experiment は OMMX Artifact family の一種として扱え、既存の Local Registry / archive / remote transport / generic Artifact inspector と互換にできる。

`OMMX Artifact v3` という media type は導入しない。v3 は SDK / 設計フェーズの名前であり、wire format の互換性境界とは分ける。将来、registry の referrers API などで Experiment だけを `artifactType` で filter したい要求が強くなった場合は、`application/org.ommx.v1.experiment` を追加で許容する余地を残す。ただし初期設計では、top-level は `application/org.ommx.v1.artifact` に統一する。

### 7.2 完全な descriptor set としての manifest

各 committed Artifact manifest は、blob bytes の保存タイミングを表すものではない。`layers[]` には、その時点の Experiment view を復元するために必要な typed payload、aggregate JSON、index JSON などの descriptor を載せる。

Instance / Solution / diagnostics などの payload blob は `log_*` 時点で Local Registry の BlobStore / draft area に逐次保存されていてよい。`commit()` が行うのは、それらの blob と、Run parameter table JSON など commit 時に materialize する payload を含む最終 Experiment state を seal し、復元に必要な descriptor set を immutable manifest として固定することである。

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
| `org.ommx.codec` | media only | external codec identifier, 必要な場合 |

Run parameter table は、必ずしも 1 parameter cell = 1 layer descriptor にならない。run parameter の key-level metadata は Run parameter table payload の内部 schema に持たせてよい。Experiment metadata を manifest annotation に物理化する場合も、`parameter` ではなく `metadata` として復元する。

Experiment name、created time、OMMX version などの experiment-level metadata は manifest annotations または dedicated metadata Record に保存する。巨大な metadata は manifest annotation に載せず Record にする。

MINTO 由来の `org.minto.*` annotation は新規書き込みでは使わない。既存 MINTO artifact の import compatibility が必要なら、compat loader が `org.minto.*` を読んで `org.ommx.*` Record model に変換する。

### 7.4 Artifact からの復元

`Experiment.load(...)` は Artifact を読み、layer annotations から immutable Experiment view を復元する。

復元に必要な invariants:

- `org.ommx.experiment.space` がない OMMX layer は Experiment Record ではないため無視してよい。
- run id は 0-based integer とし、欠番がある場合は empty run view を作るか、strict mode で error にする。
- 同一 `(space, run_id, kind, name)` の Record が複数 layer に現れた場合、manifest order の最後を採用するか error にするかを決める必要がある。提案としては committed manifest では重複を禁止し、loader は error にする。
- Run parameter table で同一 `(run_id, parameter_name)` が複数値を持つ場合も、committed manifest では重複を禁止し、loader は error にする。

## 8. Lineage モデル

Artifact lineage は OCI v1.1 `subject` で表す。v3 初期は single-parent linear history のみを扱う。

| API | 方針 |
|---|---|
| `parent()` | `subject` を読む。0/1 件 |
| `history()` | `subject` chain を root 方向に辿る |
| `diff(other)` | Record set、Run parameter table、layer descriptor を比較する |

`subject` は provenance / lineage 用リンクであり、Artifact 復元に必須の dependency ではない。各 manifest は復元に必要な descriptor set を持つので、単一 Artifact archive は parent chain なしで読める。

保存済み Experiment に run を追加する場合は、loaded `Experiment` をその場で mutable にせず、parent Artifact から派生した新しい builder を作る。`commit()` では新しい manifest を作り、`subject` に parent manifest descriptor を記録する。既存 run の descriptor / blob は再利用し、新しい run で追加された Instance / Solution / diagnostics / aggregate JSON だけが BlobStore に追加される。

同じ仕組みで、派生した Experiment version から run を削除する操作も表現できる。削除は既存 blob を消す操作ではなく、新しい manifest からその run に対応する descriptor、Run parameter table row、index record を省く操作である。元 Artifact は immutable な parent として残り、削除された run の blob は parent からは引き続き参照され得る。物理的な blob 削除は GC の到達可能性解析と retention policy に委ねる。

複数 experiment の統合は lineage merge としては扱わない。必要なら新規 Artifact の Record として入力 Artifact digest を列挙する。これは parent ではなく data reference である。

Referrers API を使った child listing は初期必須 API にしない。remote registry compatibility に依存するため、manifest と `subject` だけで完結する parent 方向の走査を先に安定させる。

## 9. Garbage Collection

`ommx artifact gc` 相当の command と、到達可能性解析に必要な API hook を提供する。

GC roots:

- Local Registry refs
- user-specified protected digests
- active autosave / draft sessions
- protected root から辿れる `subject` chain

Local Registry GC:

- IndexStore の manifest / blob records から到達可能性を解析する。
- BlobStore に存在するが IndexStore から参照されない blob は orphan blob として扱う。
- `log_*` 時点の spill や autosave draft によって作られたが、final `commit()` に到達しなかった blob / draft metadata を削除候補にする。
- 派生 Experiment version で run が削除されても、parent Artifact が root または protected subject chain から到達可能なら、その run の blob は保持される。storage reclaim は parent lineage の retention / pruning policy と組み合わせて行う。
- publish / autosave 途中の blob を誤削除しないよう grace period を置く。
- IndexStore record があるが BlobStore に bytes がない場合は corruption として report する。

Archive / exported OCI directory:

- manifest / index / explicit root digest から到達可能 blob を辿る。
- 未到達 blob を削除候補にする。

Remote registry:

- registry 実装ごとに deletion / retention policy が異なるため、v3 初期は到達可能性解析と削除候補の列挙を優先する。
- 実削除は registry capability を検出できる場合だけ行う。

GC は data model を変えない。完全な descriptor set を持つ manifest、digest primary、single-parent lineage、Record model / Run parameter table とは独立した maintenance operation とする。

## 10. 未決定事項

実装前に決める必要がある点:

1. `Experiment` の Python module path と API 名
   `ommx.experiment.Experiment` とするか、`ommx.artifact.Experiment` 配下に置くか。

2. Context manager の commit semantics
   `with Experiment(...)` 正常終了時に自動 commit するか、autosave のみ行い `commit()` は常に明示にするか。

3. Autosave storage
   Local Registry draft として実装するか、SDK-owned working directory として実装するか。

4. Record / Run parameter の物理化境界
   Record payload を `log_*` 時点で BlobStore に保存するか、commit まで payload を保持して Seal phase で descriptor 化するか。Run parameter table をどの aggregate JSON / index JSON に materialize するか。

5. Duplicate Record / parameter handling
   Build phase の upsert は許容するが、committed manifest に Record 重複や parameter key 重複を残すか、Seal 時に正規化して 1 値にするか。

6. Run deletion と lineage retention
   派生 Experiment version で run を省く API をどの範囲で提供するか。また、削除された run の blob を物理的に reclaim するための parent lineage pruning / retention policy をどう設計するか。

7. Adapter diagnostics protocol
   `SolveRecord` / `SampleRecord` のように戻り値を拡張するか、既存 `solve()` / `sample()` とは別の optional method にするか。

8. Table extraction の責務
   summary extraction を Rust core に持つか、Python-only view に寄せるか。

9. Trace provider setup UX
   Experiment core は global provider を install しない方針で固定する。一方、notebook helper / magic が UX のため provider を install することを許すか。

10. Legacy MINTO artifact import
   `org.minto.*` annotation を読む compatibility loader を OMMX に持つか、別 migration tool にするか。

## 11. 実装トラック

### Track A: Experiment / Run / Record model の中核

- `Experiment`, `Run`, immutable loaded `ExperimentView` を設計する。
- Record model、Run parameter table、typed storage API を実装する。
- `EnvironmentInfo` を first-class Record として実装する。
- Build phase upsert と Seal phase normalization を実装する。
- Python tests で MINTO の主要 UX を再現する。

### Track B: Artifact への写像と table view

- Record と Run parameter table を Artifact layer / aggregate JSON に materialize する annotation schema を実装する。
- Artifact から Experiment view を復元する loader を実装する。
- `get_run_table()` / experiment-level table view を実装する。
- `org.minto.*` compatibility の扱いを決め、必要なら import path を実装する。

### Track C: OTel trace integration

- Experiment / Run / solver / build / load / push span schema を実装する。
- `log_*` record / parameter event を実装する。
- `trace="auto" | "required" | False` を実装する。
- global `TracerProvider` を暗黙に設定しないことを tests で固定する。

### Track D: Trace layer と renderer

- OTLP JSON trace layer を Artifact に埋め込む。
- `artifact.get_trace() -> TraceResult` を実装する。
- post-hoc text tree renderer を Experiment style に対応させる。
- Chrome Trace Event Format export を derived view として提供する。

### Track E: Lineage API

- `subject` を使った `parent()` / `history()` を実装する。
- Record set と Run parameter table に基づく `diff(other)` を実装する。
- single-parent linear history の制約を tests で固定する。

### Track F: GC

- reachability analysis hook を実装する。
- Local Registry GC の dry-run / report / delete flow を実装する。
- archive / OCI directory GC の候補列挙を実装する。
- remote registry は capability detection と dry-run を優先する。
