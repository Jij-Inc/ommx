# OMMX Artifact v3 Design

OMMX Artifact v3 の設計決定をまとめる内部文書。議論ログではなく、実装に入るための合意済み方針を記録する。実装完了後は本ファイルを削除し、内容を通常の Sphinx documentation / API reference に統合する。

`ocipkg` 置換、Local Registry、minto 由来機能の取り込み範囲、OTel 統合、trace layer、lineage、tag / digest、Garbage Collection の扱いは本書の方針で固定する。

## 1. 最終方針

v3 における Artifact の最終形は以下とする。

1. `ocipkg` 依存を撤去する。ただし `ocipkg` の archive / dir / remote 共通抽象を OMMX に丸ごと吸収しない。外部 OCI crate の利用候補は remote registry transport に限定し、Artifact manifest semantics、archive materialization、Local Registry は OMMX-owned implementation とする。
2. 実験管理機構 (`Experiment`, `Run`, `DataStore`, `EnvironmentInfo`, table/export) は OMMX が直接提供する。ただし `jijmodeling` 依存は OMMX core に入れない。
3. DataStore / Artifact は記録データの source of truth、OTel は実行テレメトリの source of truth とする。
4. `MintoLogger` 相当の出力は OTel span / event の renderer として実装する。独立した logger class は作らない。
5. Artifact は build 時の trace body を self-contained layer として持てる。Phase 1 は OTLP JSON trace layer、Phase 2 以降で Logs / Metrics や scoped streaming renderer を拡張する。
6. OMMX は global `TracerProvider` を暗黙に設定しない。trace capture は `trace="auto"` を既定とし、明示要求の `trace="required"` / `with_trace()` だけを fail fast にする。
7. Artifact lineage は OCI v1.1 `subject` で表す。v3 初期は単一 parent の linear history に限定し、merge commit 相当は後続拡張に回す。
8. 各 manifest は full snapshot とする。`subject` は provenance / lineage 用リンクであり、artifact 復元に必須の dependency ではない。
9. Artifact の primary identifier は digest とする。tag は Local Registry / remote registry 上の mutable ref alias に限定する。
10. `history()`, `parent()`, `diff(other)` 相当の lineage 走査 API は提供する。子一覧取得は Referrers API 依存が強いため初期必須 API にしない。
11. Local Registry は path / tag ごとの OCI dir ではなく、IndexStore (SQLite / PostgreSQL) と BlobStore (filesystem / GCS 等) の組として定義する。`index.json` は import / export / archive 用に materialize するもので、Local Registry の mutable index にはしない。
12. SQLite Local Registry の native manifest format は OCI Image Manifest (`application/vnd.oci.image.manifest.v1+json`) のみとする。OMMX 固有 artifact の type 識別は OCI 1.1 推奨パターンに従い、manifest の `artifactType` field と `application/vnd.oci.empty.v1+json` empty config descriptor で行う。OCI Image Spec v1.1 で deprecated / removed 化された OCI Artifact Manifest (`application/vnd.oci.artifact.manifest.v1+json`) はサポートせず、生成・読み込みのいずれの経路でも扱わない。
13. 外部 OCI 形式 content の取り込みは「import」操作で行い、manifest bytes と digest を保持する identity-preserving な動作とする。import の副作用として format 変換は行わない。
14. `git gc` 相当の `ommx artifact gc` command を提供する。到達可能性解析に必要な hook も Artifact API 側に用意する。

## 2. 背景

OMMX Artifact は OCI Image / Artifact 仕様に乗せて最適化問題・解・実験メタデータを配布する仕組みである。現在は外部 crate `ocipkg` に依存している。一方、実験トラッキング層である `minto` は `ommx>=2.0.0` の Artifact API に依存しながら、Experiment / Run / DataStore 階層、provenance 収集、DataFrame 集計、階層的 console output といった「Artifact に居場所があるべき機能」を Python 側で抱えている。

Artifact v3 では、この構造を整理する。

- `ocipkg` の責務を分解し、remote OCI registry transport 以外は OMMX の Artifact model に合わせて所有し直す。
- Local Registry を shared / cloud-backed storage としても成立する形に設計し直す。
- minto の汎用実験管理機構を OMMX に吸収する。
- domain-specific な problem generator や `jijmodeling` storage は OMMX core から除外する。
- 実験の可視化と実行履歴は OTel の span / event / resource に正規化する。

## 3. 現状

### 3.1 Rust Artifact 実装

現在の Rust 実装は `rust/ommx/src/artifact.rs` と `rust/ommx/src/artifact/` にある。

| モジュール | 役割 |
|---|---|
| `artifact.rs` | registry 管理、image load / pull、layer 取得、Solution / Instance / SampleSet 抽出 |
| `builder.rs` | archive / dir backend に対する Builder trait |
| `annotations.rs` | Instance / ParametricInstance / Solution / SampleSet 用 metadata annotation |
| `media_types.rs` | OMMX 固有 media type 定義 |
| `config.rs` | config 構造体 |

`rust/ommx/Cargo.toml` は `ocipkg` に依存している。

```toml
ocipkg = { version = "0.4.0", default-features = false }

[features]
default = ["remote-artifact"]
remote-artifact = ["ocipkg/remote", "built"]
```

### 3.2 `ocipkg` 利用面

| `ocipkg` API | 用途 |
|---|---|
| `ImageName` | image reference の parse、local save path 算出 |
| `Image` trait | archive / dir / remote backend の抽象化 |
| `OciArtifact<Base>` | manifest / config / layer / blob 読み出し |
| `OciArchive(Builder)` | tar.gz 形式の `.ommx` file |
| `OciDir(Builder)` | local registry directory |
| `Remote(Builder)` | OCI Distribution API client |
| `ImageManifest` / `Descriptor` / `Digest` / `MediaType` | OCI 標準型 |
| `image::copy()` | backend 間の artifact copy |

実装上の依存は artifact 周辺に寄っているが、public surface には漏れている。

- Rust SDK は `ommx::ocipkg` を re-export している。
- Rust artifact API は `Descriptor` / `Digest` / `MediaType` を public signature に含む。
- Python `Descriptor` は `oci-spec` の JSON shape を public API として見せている。

したがって `ocipkg` 削除は内部差し替えだけでは終わらない。v3 では Descriptor / Digest / MediaType / ImageReference を OMMX-owned public types として用意し、migration note を用意する。`oci-spec` 由来型を使う場合も internal serde helper に閉じ、public API には出さない。

### 3.3 Python Artifact 実装

`python/ommx/src/artifact.rs` は PyO3 wrapper として `PyArtifact` / `PyArtifactBuilder` を提供している。

現在の主要 API:

- `Artifact.load_archive(path)`
- `Artifact.load(image_name)`
- `Artifact.push()`
- `ArtifactBuilder.new_archive_unnamed(path)`
- `ArtifactBuilder.new_archive(path, name)`
- `ArtifactBuilder.new(name)`
- `ArtifactBuilder.temp()`
- `ArtifactBuilder.for_github(org, repo, name, tag)`
- `add_*` / `get_*`: instance, solution, parametric_instance, sample_set, ndarray, dataframe, json, generic layer

v3 では Rust / Python ともに破壊的変更を許容する。既存 API を維持するより、Builder と read-only View の分離、digest primary の参照、DataStore / Experiment の一貫性を優先する。

### 3.4 Local Registry

現在の Local Registry は、image name を path に encode し、その path ごとに独立した OCI Image Layout directory を置く構造である。

- root は `set_local_registry_root(...)`、`OMMX_LOCAL_REGISTRY_ROOT`、OS default data dir の順で決まる。
- `ArtifactBuilder.new(image_name)` は `root / image_name.as_path()` に `OciDirBuilder` で直接書く。
- `Artifact.load(image_name)` はまず local path の存在を見て、なければ remote から pull して local path に置く。
- `get_images()` は root 以下を走査し、`oci-layout` を持つ directory を image として扱う。

この構造は単純だが、v3 の Local Registry には合わない。

- tag ごとに OCI dir が分かれるため、同じ blob が重複しやすい。
- 一覧取得が filesystem / object storage の full scan になる。
- writer が最終 directory に直接書くため、reader が partially written artifact を観測し得る。
- shared filesystem や mounted object storage 上で atomic update と multi-writer coordination を扱いにくい。

v3 ではこの現行 layout を legacy local registry layout として扱い、新規書き込み先ではなく read / import 互換の対象にする。Local Registry の内部表現は IndexStore + BlobStore に置き換えるが、既存 root に保存済みの path/tag OCI dir artifacts は v3 でも読み込めなければならない。

### 3.5 テスト状況

Artifact 自体の test coverage は薄い。

- Rust: `examples/create_artifact.rs`, `examples/pull_artifact.rs` の ad-hoc coverage が中心。
- Python: `python/ommx-tests/tests/test_descriptor.py` が中心。

v3 実装では Rust integration test と Python round-trip test を最初に整備する。

## 4. スコープ

### 4.1 対象

- `Cargo.toml` から `ocipkg` を削除し、`ocipkg` の archive / dir / remote 共通抽象を分解する。
- `ocipkg` / OCI public type の migration 方針を明示する。
- remote OCI registry transport は既存 crate (`oci-distribution` / `oci-client` 等) の利用を評価し、使える部分だけ採用する。
- Artifact manifest semantics、archive materialization、explicit OCI directory import/export、legacy layout migration は OMMX-owned implementation として設計する。
- Local Registry を IndexStore + BlobStore として再設計する。
- Local Registry の query / resolve / atomic publish API を用意し、`get_image_dir()` 依存を legacy local registry layout の read / import path に閉じる。
- OMMX core に `Experiment`, `Run`, `DataStore`, `EnvironmentInfo`, table/export を設計し直して取り込む。
- Artifact と OTel の双方向接続を実装する。
- build trace の self-contained trace layer を Artifact に埋め込む。
- OCI v1.1 `subject` による single-parent linear lineage を実装する。
- `history()`, `parent()`, `diff(other)` 相当の lineage 走査 API を提供する。
- `ommx artifact gc` 相当の Garbage Collection command と、到達可能性解析に必要な API hook を提供する。
- Artifact の Rust / Python tests を追加する。

### 4.2 対象外

- `ommx.v1` の Instance / Solution / SampleSet schema 変更。
- 既存 `.ommx` file の OCI Image Layout compatibility を壊す変更。
- `minto.problems.*` の problem generator 取り込み。
- `jijmodeling` への OMMX core dependency。
- `minto.datastore.ProblemStorage` の OMMX core 取り込み。
- v3 初期での multi-parent merge lineage。
- v3 初期での Referrers API based child listing。
- v3 初期での OTel Logs / Metrics の Artifact 埋め込み。

### 4.3 互換性スタンス

`minto` API compatibility は維持しない。`minto.Experiment` / `minto.Run` の class hierarchy や method signature をそのまま OMMX に持ち込む必要はない。

維持するのは user experience である。つまり、ユーザが実験を作り、run を回し、parameters / solutions / artifacts / environment を記録し、Artifact として配布し、再ロードして解析できることを保証する。

OMMX の公式 documentation / API reference では minto に言及しない。minto は設計上の参考元であり、公開 API の前提ではない。

## 5. OCI 実装方針

### 5.1 外部 crate の適用範囲

v3 では `ocipkg` の実装を丸ごと OMMX に吸収しない。Local Registry が OCI Image Layout ではなく IndexStore + BlobStore になるため、`ocipkg` の archive / dir / remote backend を同じ trait で扱う抽象は v3 の中心 model と合わない。

外部 OCI crate に期待する範囲は remote registry transport に限定する。

| 領域 | 方針 |
|---|---|
| remote manifest GET / PUT | `oci-distribution` / `oci-client` 等の利用を評価する |
| remote blob upload / download | 既存 crate を優先する |
| auth / credential handling | 既存 crate を優先する |
| cross-repository mount | 既存 crate が扱えるなら利用する |
| Referrers API | 既存 crate が扱えるなら利用する |

OMMX-owned implementation とする範囲:

- OMMX Artifact manifest / config / layer semantics。
- Descriptor / digest / media type の public API 方針。
- Local Registry の IndexStore / BlobStore。
- atomic publish と GC。
- `.ommx` archive import / export。
- explicit OCI directory layout import / export。
- legacy path/tag OCI dir layout migration。
- image reference の public API。

`ocipkg` は現在すでに `oci-spec` を利用しているため、「OCI 標準型が自前実装だから置き換える」という整理ではない。v3 の主眼は、`ocipkg` の local layout / copy abstraction を再実装することではなく、Artifact の永続化 model を Local Registry、archive/export、remote transport に分解することである。

### 5.2 公開 API surface

v3 では `ommx::ocipkg` re-export を削除する。`Descriptor` / `Digest` / `MediaType` / `ImageReference` は OMMX-owned public types として整理する。

`oci-spec` 型を直接 public API として採用しない。利用する場合も JSON schema / serde helper として内部に閉じる。Python の `Descriptor` JSON shape と Rust の public signature は migration note の対象にする。

### 5.3 Registry compatibility

OCI v1.1 `subject` と Referrers API は全 registry で同じように動くとは限らない。

v3 初期では fallback 仕様を先に固定しない。archive と明示 export された OCI directory layout は完全に制御できるため `subject` をそのまま扱う。remote registry が `subject` push を拒否した場合は、annotation fallback で曖昧に継続せず、明示 error とする。実際の非対応 registry に遭遇した時点で fallback を設計する。

### 5.4 OCI Image Layout の位置づけ

OCI Image Layout (`oci-layout`, `index.json`, `blobs/`) は Local Registry の内部形式としては使わない。`index.json` を持たない directory は OCI Image Layout ではないため、v3 Local Registry の BlobStore root には `oci-layout` を置かない。

v3 における OCI Image Layout は import / export 用の interchange format である。

- `.ommx` archive の import / export。
- 明示的に export された OCI directory layout。
- legacy directory layout backend からの import (5.5 / 6.6 / 6.7 参照)。
- remote OCI registry への push / pull 境界。
- 標準 tool で検査できる interchange format。

Local Registry 内部では IndexStore が refs / manifests / entries の source of truth になり、BlobStore が content-addressed bytes の source of truth になる。filesystem / GCS BlobStore が `blobs/<algorithm>/<encoded>` という key convention を使っても、それは OCI Image Layout ではなく単なる CAS object namespace である。

標準 OCI Image Layout が必要な場合は、IndexStore + BlobStore から export 先 directory または archive に `oci-layout`, `index.json`, `blobs/` を materialize する。

### 5.5 Manifest format

SQLite Local Registry の native manifest format は OCI Image Manifest (`application/vnd.oci.image.manifest.v1+json`) のみとする。OMMX 固有 artifact の識別は manifest top-level の `artifactType` field (`application/org.ommx.v1.artifact`) で行う。parse 時に検証するのはこの field のみで、`config` blob は実装上のデフォルトを定めるだけの非識別情報とする。

OCI Image Spec v1.1 で deprecated / removed 化された OCI Artifact Manifest (`application/vnd.oci.artifact.manifest.v1+json`) はサポートしない。次の理由による。

- OCI 仕様レベル: image-spec 1.1 で正式に removed され、Artifact Manifest 仕様文書は archive 扱い。後継パターンは「Image Manifest + `artifactType` + empty config」。
- レジストリ実装の現実: `distribution/distribution` v2 系 (= `registry:2`) は default 設定で `application/vnd.oci.artifact.manifest.v1+json` を manifest schema allow-list に含まず、push 時に `MANIFEST_INVALID` で reject する。他レジストリでも対応に差がある。
- runtime polymorphism の不要性: format coexistence を維持するメリットより、reader / writer / test を Image Manifest 単形式に閉じた方が SDK / migration / 相互運用すべてで単純。

build / import / pull のいずれの経路でも、Local Registry に保存される manifest は Image Manifest である。import 時に旧 v2 OMMX が生成済みの Image Manifest はそのまま identity-preserving に取り込む (manifest bytes / digest を保持)。万一外部由来の Artifact Manifest を import しようとした場合は、未サポート format として明示 error にする (sloppy fallback はしない)。

native build path (v3 `LocalArtifactBuilder`) は SDK v2 の archive build path (`ocipkg::OciArtifactBuilder::new` 経由) と byte-level に整合する manifest shape を採用する。具体的には:

- `schemaVersion: 2`、`artifactType: application/org.ommx.v1.artifact`、`config` は OCI 1.1 empty descriptor (`application/vnd.oci.empty.v1+json` の 2-byte JSON `{}`)、`layers[]` は各 entry に `annotations` field を持つ (空 object でも render する)。manifest top-level の `mediaType` field は意図的に出力しない (v2 SDK と同様、HTTP Content-Type は push 時に transport が個別に付与する)。
- v3 SQLite registry が出力する manifest と v2 archive build が出力する manifest の唯一の差分は JSON field 順序で、v3 は reproducible digest のため `stable_json_bytes` で alphabetical sort する (v2 SDK は struct 宣言順)。

import 経路で v2 SDK が生成した OMMX-specific config (`application/org.ommx.v1.config+json`) を持つ legacy Image Manifest は、parse-time check が `artifactType` だけなので識別 / read に支障なく取り込める。

format 変換のための「convert」操作は v3 では提供しない。v2 で生成された Image Manifest と v3 で生成される Image Manifest は同じ format であり、`config` blob の中身が違うだけで format conversion API は不要である。

## 6. Local Registry model

### 6.1 役割

Local Registry は named Artifact の永続 store / cache / checkout area である。local filesystem の開発者 cache だけでなく、shared filesystem や cloud-backed blob store 上で複数 process / runner が同時に読み書きする用途を想定する。

v3 では Local Registry を path / tag ごとの OCI dir とは定義しない。Local Registry は次の 2 層からなる。

| 層 | 役割 |
|---|---|
| IndexStore | image name / tag / digest / manifest / DataStore entry metadata の queryable index |
| BlobStore | digest-addressed bytes の保存先 |

### 6.2 IndexStore

IndexStore は Local Registry の mutable state を持つ。実装は storage profile ごとに差し替える。

| 実装 | 用途 |
|---|---|
| SQLite | single-node local cache、test、CLI workflow |
| PostgreSQL | shared registry、multi-node writer、cloud deployment |

SQLite は同一 node 上の複数 process / runner が同じ local cache に短時間 write する用途を許容する。この場合、write は SQLite の transaction によって serialize される前提とし、高頻度 writer や長時間 transaction を持つ shared registry にはしない。SQLite file を mounted object storage 上の multi-writer registry として使わない。shared filesystem、multi-node writer、cloud deployment では PostgreSQL 等の transaction を持つ外部 database を使う。

IndexStore が持つ最小情報:

| テーブル相当 | 内容 |
|---|---|
| refs | image name + tag/digest reference -> manifest digest |
| manifests | manifest digest -> media type, size, subject, annotations, created_at |
| manifest_layers | manifest digest -> layer descriptors |
| blobs | content digest -> size, media type, storage URI, kind, last verified time |
| entries | DataStore entry name/type -> descriptor, manifest digest, query metadata |

`entries` は Artifact の source of truth ではなく query index である。Artifact の完全な復元は manifest と referenced blobs から可能でなければならない。

### 6.3 BlobStore

BlobStore は content-addressed bytes を保存する。対象は layer payload だけではなく、config bytes、manifest JSON bytes、trace layer など Artifact を構成するすべての content-addressed object である。filesystem backend では `blobs/<algorithm>/<encoded>`、GCS backend では同じ logical key を object name として使う。

この `blobs/` は OCI Image Layout の `blobs/` と同じ digest-addressed naming を借りるだけで、BlobStore root 自体は OCI dir ではない。BlobStore root には `oci-layout` も `index.json` も置かない。

BlobStore の規則:

- write は digest-addressed で idempotent にする。
- 既存 digest に異なる bytes を書こうとした場合は error にする。
- read 時は必要に応じて size / digest を検証する。
- query / listing は BlobStore に依存しない。listing は IndexStore で行う。

### 6.4 Atomic publish

ここで使う「publish」は OCI Distribution 由来の用語であり、**registry 側から見た動詞**として一貫して使う。具体的には、Local Registry が manifest を **receive** して、対応する content-addressed blobs と一緒に IndexStore に登録し、`image_name` で指定された ref を新しい manifest digest に立てて、その artifact を **discoverable な状態にする** 一連の atomic operation を指す。

Git で例えるなら `git commit` の内部実装に相当する: `.git/objects/` に object を書き、`refs/heads/<branch>` を新しい commit digest に進める。SDK の Build / Seal / View 三相 (7.4 章) における Seal フェーズの I/O 部分は、ここで定義する registry publish primitive を呼ぶ。

`ArtifactBuilder.build()` が「commit する」、`LocalRegistry::publish_artifact_manifest` が「registry 側で receive して ref を立てる」、という二つの異なるレイヤーの動詞であり、ユーザ視点での `commit / build` と registry 視点での `publish` を意図的に分けている。

DB と BlobStore は分散 transaction にならないため、publish 順序を固定する。

1. layer / config / manifest bytes を build phase で作る。
2. digest と size を計算する。
3. BlobStore に layer / config / manifest などの content-addressed objects を idempotent upload する。
4. IndexStore transaction で blobs / manifest / entries / ref を insert または update する。
5. transaction commit 後に artifact を visible とする。

途中で失敗した場合、IndexStore に commit されていない artifact は見えない。BlobStore に残った unindexed blob は GC で回収する。

tag update は IndexStore transaction 内の ref update として扱う。`ArtifactBuilder.build()` が最終 path に直接書く方式は v3 Local Registry では使わない。

並行 publish では、unique ref / digest への書き込みは互いに独立して成功できる。同じ mutable ref へ異なる manifest digest を publish する場合、Local Registry は少なくとも次の primitive を提供する。

| primitive | 方針 |
|---|---|
| keep-existing publish | 既存 ref を保持し、異なる digest がすでに publish されていれば conflict とする |
| replace publish | 呼び出し側が明示した場合だけ ref を新しい digest に更新する |

`latest`, `active`, `current run` のような alias に last-writer-wins、compare-and-swap、promote-only のどれを採用するかは Experiment / Run 層の semantic として決める。Local Registry 層はこれらを実現するための atomic publish primitive を提供するに留める。

### 6.5 Read / query API

v3 の Local Registry API は path ではなく reference / descriptor / blob reader を中心にする。

| API | 方針 |
|---|---|
| `Artifact.exists(ref)` | IndexStore で ref を解決できるかを返す |
| `Artifact.resolve(ref) -> Descriptor` | tag / digest reference を manifest descriptor に解決する |
| `Artifact.load(ref)` | manifest descriptor と referenced blobs から read-only view を作る |
| `Artifact.list(prefix=...)` | IndexStore query。BlobStore / filesystem scan はしない |
| `Artifact.open_blob(digest)` | 内部用 blob reader。digest / size verification を行う |

`get_image_dir(image_name)` は v3 の中心 API ではない。OCI dir backend 互換や migration tool のための legacy API とし、Local Registry の existence check / listing / read path には使わない。

ただし既存 Local Registry の read 互換は維持する。legacy local registry layout (`get_image_dir(ref)` が指す path/tag OCI dir 群) は、ユーザーが明示的に `ommx artifact import` を実行するか、Rust / Python SDK の import API を呼び出したときに、manifest / descriptors / blobs を検証して **manifest bytes と digest を保持したまま** IndexStore + BlobStore に取り込む (5.5)。

「ref miss のたびに legacy 全体を再探索しない」原則は、**root の再帰 scan のような重い処理を回避する** という意味で読む。`Artifact.list` が IndexStore のみを query するのはこの原則による。一方で、`Artifact.load(image)` のような単一 image を要求する高レベル API は、SQLite miss 時に **その image の legacy path 一つに対する `Path::exists()`** で probe し、見つかれば 1 回だけ自動 import して以後の呼び出しは IndexStore fast path に乗せる「lazy auto-migration」を許容する。これは v2 ユーザの ergonomics を壊さないための例外で、

- probe は対象 image 一つの dir 存在判定だけ (root scan ではない)
- 一度 import すれば 2 回目以降は SQLite hit、再 probe は走らない
- legacy data からの **読み出しではなく、SQLite への取り込み**(identity-preserving import)

という条件を満たす範囲に限る。read を fallthrough する fallback (毎回 legacy を読む) は依然として禁止。`Artifact.list` のような listing API も依然 SQLite のみ。

import 時に新 Local Registry 側へ同名 ref がすでに存在し、legacy 側と manifest digest が異なる場合、default は既存 ref を保持して当該 entry を skip する。置換は `ommx artifact import --replace`、または SDK の import API で `Replace` policy を明示した場合だけ行う。同名 ref が同じ digest を指している場合は conflict ではなく、manifest / blobs の存在確認と再登録を行う idempotent verify として扱う。

並行 import では、default policy は ref publish を atomic insert として扱う。同じ legacy ref / digest を複数 process が同時に import した場合、最初の publish が成功、後続は verify になる。異なる digest が同じ ref に並行 publish される場合、default は first writer wins で後続を conflict skip とする。`--replace` は明示的な destructive operation なので、並行実行時は last writer wins とする。BlobStore は CAS path へ直接 partial write せず、同一 directory 内の temporary file に書いてから atomic publish する。

### 6.6 Import / export

OCI Image Layout との互換は import / export boundary で保つ。

- import: 外部の OCI 形式 content を **manifest bytes / 各 blob を bytes そのまま** 検証して IndexStore + BlobStore に登録する。manifest format は OCI Image Manifest のみを受け入れる (5.5 参照)。format 変換はしない (bytes と digest を保持する)。外部由来の Artifact Manifest は未サポート format として明示 error にする。対応する source は次の 4 つで、すべて同じ identity-preserving rule に従う:
  - 単一の OCI Image Layout directory (`oci-layout` + `index.json` + `blobs/`)。`oras` / `crane` / `skopeo` 出力でも v2 OMMX local registry の path/tag entry でも同様。
  - v2 OMMX local registry layout (path/tag tree)。再帰 scan で root 下の OCI dir を列挙し、上記の per-dir import を batch で適用する。
  - `.ommx` OCI archive (tar.gz)。
  - remote OCI registry からの pull。manifest / blobs を BlobStore に入れ、IndexStore transaction で ref を登録する。
- default export: 指定された manifest descriptor 1 つを root にして、その manifest の material closure を集め、standard OCI Image Layout (`oci-layout`, `index.json`, `blobs/`) を materialize する。Git で言えば `depth=1` の export である。
- history bundle export: 明示 opt-in。指定された manifest から `subject` chain を辿り、lineage closure も同じ archive / directory に materialize する。Git で言えば `--depth=N` または full history bundle に相当する。offline で `history()` を使いたい場合の形式であり、default `.ommx` export とは分ける。
- remote push: IndexStore + BlobStore から manifest / blobs を読み、OCI Distribution API に送る。

この方針により、Local Registry は queryable / transactional な内部 store として実装しつつ、`.ommx` file と remote OCI registry との互換性を維持する。

Export closure の定義:

| closure | 含むもの | 用途 |
|---|---|---|
| material closure | root manifest、`config` blob (empty config を含む)、`layers[]` の descriptor、trace layer など、その manifest を読んで snapshot を復元するために必要な content-addressed objects | default export |
| lineage closure | `subject` chain で到達する parent manifests と、それぞれの material closure | history bundle export |

`subject` は default export において descriptor として manifest 内に残るが、material closure には含めない。したがって parent digest は分かるが、parent manifest / parent blobs は archive 内に存在しない場合がある。

## 7. DataStore / Experiment model

### 7.1 DataStore の構造

minto `DataStore` は名前付きの型別 dict を束ねた構造である。v3 では OMMX core の DataStore を次のように整理する。

| カテゴリ | 種別 | Artifact manifest mapping |
|---|---|---|
| per-entry storage | instances, solutions, samplesets, objects, generic media entries | 名前ごとに 1 layer |
| normalized scalar storage | parameters, metadata | key/value ごとに 1 layer |
| environment storage | EnvironmentInfo | first-class artifact entry |

現 minto の aggregate dict (`parameters`, `meta_data`) は content-addressable Artifact model と相性が悪い。key 追加のたびに dict 全体を再 encode する必要があり、append-only ではなくなるためである。

v3 では aggregate dict を per-entry に正規化する。例えば `parameters["alpha"] = 0.1` は `("alpha", 0.1)` の独立 entry になる。

### 7.2 Pluggable storage

OMMX core は generic media type storage を持つ。外部 package は media type と codec を登録することで domain-specific data を保存できる。

ただし OMMX core は `jijmodeling` を import しない。`jijmodeling` problem storage が必要なら、OMMX core ではなく optional adapter / external package が提供する。

### 7.3 EnvironmentInfo

`EnvironmentInfo` は Artifact / DataStore の first-class entry として永続化する。OTel `Resource` はその投影であり、情報本体ではない。

OTel `Resource` へ写す属性は standard semantic conventions を優先する。OS / host / process / runtime / container などは `os.*`, `host.*`, `process.*`, `process.runtime.*`, `container.*` 等に寄せる。標準で表現できない OMMX 固有情報だけを `ommx.*` namespace に置く。

同じ意味の値を標準属性と `ommx.*` に二重記録しない。

### 7.4 Build / Seal / View

Artifact の mutation semantics は 3 相に分ける。

| 相 | 性質 | API |
|---|---|---|
| Build | in-memory mutable | `ArtifactBuilder`, `Experiment`, `Run` |
| Seal | snapshot を作る | `build()` / `commit()` |
| View | immutable read-only | `Artifact` |

Build 相では同名 key の upsert を許容してよい。Seal 相で最終 DataStore view を snapshot として manifest に固定する。View 相には `add` / `update` を生やさない。

Seal の I/O 段階 — manifest と layer blobs を実際に Local Registry に書き、ref を立てて discoverable にする atomic 動作 — は registry-internal primitive である `publish_artifact_manifest` (6.4 参照) を呼ぶ。「publish」は OCI Distribution 由来の registry 側の動詞、「commit / build」は SDK ユーザが触る動詞。Git で言えば `git commit` (ユーザ視点) と "object を `.git/objects/` に書いて `refs/heads/<branch>` を進める" (内部実装) の関係に対応する。

永続層に update primitive は存在しない。永続化済み Artifact を変える唯一の方法は、新しい full-snapshot Artifact を作ることである。

### 7.5 Commit granularity

`1 manifest = 1 commit` とする。

- Core primitive は明示 `build()` / `commit()`。
- High-level `Experiment` は experiment 終了時に自動 commit する。
- `Run` 終了ごとに manifest を切る挙動は `commit_per_run=True` 相当の opt-in にする。
- Default では run ごとに commit しない。

## 8. OTel / Trace model

### 8.1 Source of truth

DataStore / Artifact と OTel の責務を分ける。

| 領域 | Source of truth |
|---|---|
| parameter / solution / sample set / object / environment | DataStore / Artifact |
| lifecycle / duration / IO / error / record reference | OTel trace |
| console rendering | OTel renderer |

`run.log_parameter(...)` や `run.log_solution(...)` は logger 呼び出しではない。一次効果は DataStore への記録であり、OTel span event は「この run で何が記録されたか」を可視化する副次的 telemetry である。

### 8.2 MintoLogger の解体

`MintoLogger` 相当の単一 class は作らない。minto の logger が混ぜていた信号を分解する。

| 元の機能 | v3 の所属 |
|---|---|
| Experiment / Run / Solver の開始終了 | OTel span |
| parameter / solution / sample set / object の記録 | DataStore entry + span event |
| warning / error / debug | OTel Logs または Rust `tracing::{warn,error,debug}!` |
| EnvironmentInfo 表示 | EnvironmentInfo entry + Resource projection + renderer |
| indent 付き console output | post-hoc / streaming renderer |

Phase 1 では OTel Logs を Artifact に埋め込まない。warning / error は span event と span status に寄せる。OTel Logs / Metrics の Artifact 埋め込みは Phase 2 以降で扱う。

### 8.3 Span hierarchy

Trace ID は OMMX が独自採番しない。OTel の root span 作成時に発行される。

Span 開始ルール:

- `Experiment` 開始時:
  - active span がなければ `ommx.experiment` root span を開始し、新しい `trace_id` が発行される。
  - active span があれば `ommx.experiment` はその child span となり、既存 `trace_id` を継承する。
- `Run` 開始時:
  - `ommx.run` は `ommx.experiment` の child span。
  - 新しい `trace_id` は発行せず、`span_id` だけ新しくなる。
- `ArtifactBuilder.build()`:
  - `ommx.artifact.build` span を開始する。
  - active span があれば child span となり、既存 `trace_id` を継承する。
  - active span がなければ root span となり、新しい `trace_id` が発行される。
- `Artifact.load*` / `push`:
  - artifact を使う側の trace として現在の active span に接続する。
  - build-time trace と同じ trace に無理に接続しない。
  - artifact 内の build-time trace は load / push span から OTel Link として参照する。
- 派生 Artifact:
  - lineage は OCI `subject` で表す。
  - trace は新しい `ommx.artifact.build` span を持つ。
  - parent artifact の build trace は、存在すれば OTel Link として張る。

Manifest annotation key:

- `org.ommx.trace.build.trace_id`
- `org.ommx.trace.build.span_id`

### 8.4 Trace layer

Artifact は build 時の trace body を dedicated layer として埋め込める。これは Cloud Run / batch job のように Artifact 入出力だけで完結する実行環境で重要である。

Phase 1 の trace layer:

| 項目 | 方針 |
|---|---|
| encoding | OTLP JSON |
| media type | `application/vnd.ommx.trace.otlp+json` |
| payload | OTLP JSON mapping の `ExportTraceServiceRequest` 互換 (`resourceSpans`) |
| 対象 signal | span / span event |
| derived format | Chrome Trace Event Format は読み出し時に生成 |
| API | `artifact.get_trace() -> TraceResult` |

Trace layer は DataStore の代替ではない。parameter / solution / sample set / environment の本体は通常の Artifact entry に保存し、trace layer は実行時系列と record reference を保存する。

### 8.5 Trace capture mode

OMMX は global `TracerProvider` を暗黙に設定しない。import、`Experiment` 開始、`ArtifactBuilder.build()` のいずれでも `trace.set_tracer_provider()` を勝手に呼ばない。

Trace capture mode:

| mode | 動作 |
|---|---|
| `trace="auto"` | default。provider / collector が設定済みなら trace layer を埋め込む。未設定なら trace layer を省略し、status annotation を残す |
| `trace="required"` | 明示要求。provider / collector が未設定なら setup error |
| `with_trace()` | 低レベル builder の明示要求。provider / collector が未設定なら setup error |
| `trace=False` | 常に trace layer を生成しない |

`trace="auto"` で trace layer を省略した場合の manifest annotations:

- `org.ommx.trace.status=not_recorded`
- `org.ommx.trace.reason=no_tracer_provider`

この設計により、通常の Experiment 利用は OTel setup を必須にせず、trace を成果物として要求する利用では欠落を fail fast で検知できる。

### 8.6 Renderer

Phase 1 は post-hoc renderer のみを提供する。

- `Experiment` / `Run` / `ArtifactBuilder.build()` が span / event を発行する。
- `capture_trace()` または Experiment 内部 collector が span を収集する。
- Experiment 終了後または Artifact load 後に `TraceResult` を作る。
- `trace_result.text_tree(style="experiment")` 相当で描画する。

Phase 2 で scoped streaming renderer を追加する。

- `Experiment(..., live=True)` 相当で opt-in。
- 対象 `trace_id` だけを購読する scoped `SpanProcessor` を、呼び出し側が設定した SDK `TracerProvider` に attach する。
- span end / event を逐次 render する。
- Experiment 終了時に processor を deactivate / detach する。

Phase 2 は span / event schema を変更せず、同じ OTel signal を読む renderer を増やす形で実装する。

## 9. Lineage model

### 9.1 Full snapshot manifest

各 manifest は full snapshot とする。派生 Artifact の `layers[]` には、その時点の DataStore view を復元するために必要なすべての descriptor を載せる。

既存 blob は同じ digest の descriptor として再利用できる。Local Registry では BlobStore の CAS として共有され、remote registry では dedup / mount され得る。一方、archive や明示 export された OCI directory layout では、その Artifact 単体で読めるよう参照 blob を含める。

`subject` は lineage / provenance のためのリンクであり、子 Artifact を読むための必須 dependency ではない。

`subject` の参照先 manifest は OCI Image Manifest であり、`mediaType` は `application/vnd.oci.image.manifest.v1+json` 固定とする。Local Registry が単一 format に閉じている (5.5) ため、format 跨ぎ dispatch は不要。

したがって単一 Artifact の archive export は、その Artifact の material closure だけを self-contained にする。これは Git の `depth=1` に近い。history に含まれる parent Artifact は default export の dependency ではなく、同梱したい場合は history bundle export を明示する。

```jsonc
{
  "schemaVersion": 2,
  "artifactType": "application/org.ommx.v1.experiment",
  "config": {
    "mediaType": "application/vnd.oci.empty.v1+json",
    "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    "size": 2
  },
  "layers": [ ... full DataStore snapshot ... ],
  "subject": {
    "mediaType": "application/vnd.oci.image.manifest.v1+json",
    "digest": "sha256:...",
    "size": 1234
  }
}
```

manifest top-level の `mediaType` field は v2 SDK と同様に出力しない (5.5)。Content-Type は push 時に transport が個別に付与する。`subject` descriptor の `mediaType` は OCI Image Manifest 固定 (9.1)。

### 9.2 Linear history

v3 初期は OCI v1.1 `subject` の単一 parent に寄せ、linear history のみを扱う。

| 概念 | Git | OCI v1.1 | OMMX |
|---|---|---|---|
| content address | blob | descriptor -> blob | Instance / Solution / SampleSet 等 |
| snapshot | tree | manifest (`layers[]`) | 1 つの experiment state |
| history node | commit with parent | manifest + `subject` | 派生 Artifact |
| mutable ref | tag / branch | tag | digest primary の ref alias |
| history traversal | `git log` | `subject` chain | `history()` |

複数 experiment の統合は lineage merge としては扱わない。必要なら新規 Artifact の DataStore entry として入力 Artifact digest を列挙する。これは parent ではなく data reference である。

多 parent が必要になった場合は、後続 version で annotation 規約または OCI 側の標準機能を再検討する。

### 9.3 Digest and tag

Artifact の primary identifier は digest とする。API / metadata / provenance で再現性が必要な場所には digest を保存する。

`experiment:latest`, `experiment:v2` のような tag は Local Registry / remote registry 上の mutable ref alias として扱う。OMMX の Experiment API では branch concept を前面に出さない。

### 9.4 Lineage API

v3 初期で提供する lineage 走査 API:

| API | 方針 |
|---|---|
| `parent()` | `subject` を読む。0/1 件 |
| `history()` | `subject` chain を root 方向に辿る |
| `diff(other)` | manifest の layer descriptor 列と DataStore entry metadata を比較する |

Referrers API を使った「この Artifact を parent に持つ子一覧」は初期必須 API にしない。remote registry compatibility に依存するため、manifest と `subject` だけで完結する parent 方向の走査を先に安定させる。

## 10. Garbage Collection

長期運用では古い manifest や未参照 blob が Local Registry / remote registry / exported layout に残る。v3 では `git gc` 相当の `ommx artifact gc` command を提供する。

GC の責務は以下とする。

- Local Registry では、IndexStore の refs、explicit protected digest、`subject` chain を root として到達可能な manifest / blob を辿る。
- BlobStore に存在するが IndexStore から参照されない blob は orphan blob として扱う。publish 途中の blob を誤削除しないよう、grace period を置いてから削除候補にする。
- IndexStore に manifest / blob record があるが BlobStore に bytes がない場合は corruption として report する。自動修復は remote / archive 等の recovery source がある場合だけ行う。
- legacy archive / OCI directory layout backend では、manifest / tag / explicit root digest から到達可能な blob を辿り、未到達 blob を削除対象にする。
- remote registry では registry 実装ごとに deletion / retention policy が異なるため、v3 初期は到達可能性解析と削除候補の列挙を優先する。実削除は registry capability を検出できる場合だけ行う。
- `subject` chain、tag alias、user-specified protected digest を GC root として扱う。
- Artifact API 側に到達可能性解析 hook を用意し、CLI command と将来の storage-specific GC が同じ解析を使えるようにする。

GC は data model を変えない。full snapshot、digest primary、single-parent lineage、IndexStore + BlobStore の方針は GC 実装と独立している。

## 11. リスク

- **OCI Distribution の実装量**: auth、manifest PUT / GET、blob upload session、cross-repo mount、chunked upload まで含めると実装量が大きい。
- **既存 `.ommx` file / 旧 Local Registry の後方互換**: 旧 v2 OMMX は OCI Image Manifest を生成しており、v3 native build と同じ format なので、import は manifest bytes / digest を identity-preserving に保持する。
- **OCI 1.1 `artifactType` への registry / tool 対応差**: SQLite Local Registry は Image Manifest 一形式に閉じるが、`artifactType` field を読み取らない古い tooling では OMMX artifact が generic Image として表示される。push 先 registry が `artifactType` を保持して serve するかは registry 実装依存で、Referrers API 等の lineage 系機能の対応差も生じる。実 registry 検証は Step B 系 PR で扱う。
- **`ocipkg` public surface の撤去**: `ommx::ocipkg` re-export、Rust / Python の `Descriptor` / `Digest` / `MediaType` 露出を置き換える必要がある。
- **IndexStore / BlobStore の整合性**: DB と object store は分散 transaction ではない。blob upload -> DB transaction commit の順序、orphan blob GC、missing blob detection を実装で徹底する必要がある。
- **SQLite の適用範囲**: SQLite は single-user local cache に限定する。mounted object storage 上の shared multi-writer registry では PostgreSQL 等を使う必要がある。
- **minto user への影響**: API compatibility は維持しないため、取り込み時期と migration messaging が必要。
- **TracerProvider 所有権**: 現行 `ommx.tracing` の lazy setup が provider を install する挙動は v3 方針と衝突するため見直す必要がある。
- **Logger output の期待差分**: Phase 1 は post-hoc 表示なので、従来の live console output を期待する環境では明示的な説明が必要。
- **Registry の OCI v1.1 対応差**: `subject` 非対応 registry に当たる可能性がある。初期方針は explicit error とし、fallback は実ケースが出てから設計する。

## 12. 実装ステータス

v3 design の実装は段階的に進める。本書は全項目が完了した時点で削除し、内容を Sphinx documentation / API reference に統合する。後続 PR の TODO は PR description ではなく本章に集約する (merged PR は後から検索しづらく、本書は branch を跨いで update できるため)。

### 12.1 SQLite Local Registry foundation (landed)

v3 PR #864 で landing する範囲。

- SQLite-backed Local Registry (`SqliteIndexStore` + filesystem CAS `FileBlobStore`)。`Mutex<Connection>` で `Sync` を満たし、poisoned mutex は `into_inner()` で recovery (panic させない)。
- `ArtifactBuilder.new(image).build()` と `Artifact.load(image)` を SQLite registry 経由に切り替え。
- legacy OCI dir からの identity-preserving import (`ommx artifact import` CLI + SDK の `import_legacy_local_registry*` / `import_oci_dir*`)。旧 v2 OMMX が生成した OCI Image Manifest は v3 と同 format なので bytes / digest を保持してそのまま登録できる。
- `Artifact.load(image)` / `ommx load` の lazy auto-migration (6.5)。
- ocipkg seam の局所化: 残った ocipkg 依存は `local_registry::import::archive` と `local_registry::import::remote` の 2 モジュールのみ。
- 並行 publish primitive (`RefConflictPolicy::{KeepExisting, Replace}`、`RefUpdate::{Inserted, Unchanged, Replaced, Conflicted}` 4 状態)。
- `import::archive` / `import::remote` は `registry.root().join(image_name.as_path())` に staging する (`get_image_dir` の global default に縛られない)。

PR #868 で本書の Image Manifest 一本化方針 (5.5) と Step A → B' → B → C milestone 構成 (12.3) も merged。design doc 上は Image Manifest only に統一されたが、#864 / #866 が landing した時点では `LocalArtifactBuilder` は依然として deprecated 化された OCI Artifact Manifest を生成しており (`LocalManifest::Artifact` variant、`OCI_ARTIFACT_MANIFEST_MEDIA_TYPE`、`ensure_ommx_artifact_manifest` validator 等が残存)、これらの撤去は §12.3 Step B' で扱う。

### 12.2 後続 PR に残す TODO

| 項目 | 仕様参照章 | 現状の注記 |
|---|---|---|
| `import::archive` / `import::remote` の stage 1 を ocipkg ベースから native streamer に置換 | 5.1, 6.6 | 現状は `ocipkg → legacy OCI dir → import_oci_dir_as_ref → SQLite` の 2-stage。public 関数 signature は変えない |
| `ommx::ocipkg` re-export と、Rust / Python の `Descriptor` / `Digest` / `MediaType` / `ImageReference` public surface の撤去 | 5.1, 5.2 | OMMX-owned public types への置き換え。migration note を別途用意 |
| `LocalArtifactBuilder` を OCI Image Manifest with `artifactType` + empty config に refactor | 5.5 | 現状は deprecated 化された OCI Artifact Manifest (`application/vnd.oci.artifact.manifest.v1+json`) を生成しており、`distribution/distribution` v2 系で `MANIFEST_INVALID` reject される。`LocalManifest::Artifact` variant / `OCI_ARTIFACT_MANIFEST_MEDIA_TYPE` 関連 dead code もこの PR で撤去 (§12.3 Step B')。**Landed in PR #869.** |
| Archive build path (`ArtifactBuilder.new_archive*`, `temp()`, `Artifact.load_archive`) の v3 pipeline 化 | 5.5, 7.4 | 現状は ocipkg-based Image Manifest pipeline。SQLite registry を経由しない。v3 native build と同じ Image Manifest with `artifactType` を生成する |
| SQLite registry から remote への native `push` | 6.4, 6.6 | 現状の Python `Artifact.push()` は legacy OCI dir 経由の transitional path。v3-native build artifact (legacy 不在) は明示 error。transport crate は `oci-client` (ORAS, [oras-project/rust-oci-client](https://github.com/oras-project/rust-oci-client)) を採用予定 |
| `Artifact.load(image)` / `ommx load` の legacy double-write 撤廃 | 6.5 | `push` / `save` / Python archive 読み出しが SQLite から直接読めるようになり、かつ native stage 1 が landing したら |
| Lineage API (`parent()`, `history()`, `diff(other)`) | 9.4 | full snapshot + `subject` chain 前提。child 一覧 (Referrers API) は初期対象外 |
| OTel trace layer 埋め込み | 8.4, 8.5 | Phase 1 は OTLP JSON のみ。global `TracerProvider` は設定しない (`trace="auto"`/`"required"` mode) |
| Trace renderer | 8.6 | Phase 1 は post-hoc renderer、Phase 2 で scoped streaming |
| `ommx artifact gc` と reachability analysis hook | 10 | Local Registry / 旧 archive / OCI directory layout を対象。remote registry は capability 検出できる場合のみ実削除 |
| `rust/dataset/{miplib2017,qplib}` packaging path の v3 化 | 6.6 | 新 namespace `ghcr.io/jij-inc/ommx/v3/{miplib2017,qplib}:*` を OCI Image Manifest with `artifactType` で publish する。既存 `ghcr.io/jij-inc/ommx/{miplib2017,qplib}:*` (v2 OMMX 生成の Image Manifest) は freeze し touch しない。code 切替は #866 (Step A) で完了したが、当時の builder が deprecated Artifact Manifest を生成していたため、初回 publish は §12.3 Step B' (builder refactor) と Step B (native push) の両 landing 後 |
| DataStore / Experiment / Run / EnvironmentInfo の OMMX core 取り込み | 7 | minto-equivalent functionality を OMMX-owned で再設計。API compat は破棄 (4.3) |

### 12.3 次の実装 milestone (A → B' → B → C)

§12.2 のうち、新機能ではなく構造を片付ける項目を以下の順序で進める。各 step は独立 PR、step 間で SDK が壊れない状態を維持する。

**Step A — legacy ocipkg-based dataset build path の撤去** (§12.2 行 10 code 部分)。**Landed in PR #866.**

`rust/dataset/{miplib2017,qplib}` を `LocalArtifactBuilder::new` + 明示的 `add_source` に切り替え、出力 image name を `ghcr.io/jij-inc/ommx/v3/{miplib2017,qplib}:<tag>` とする。唯一の caller が消えた `rust/ommx/src/artifact/builder.rs` の `Builder<OciDirBuilder>::{new, for_github}` を削除。既存 `ghcr.io/jij-inc/ommx/{miplib2017,qplib}:*` は触らない (freeze) — SQLite registry は両 namespace を identity-preserving に保持できるため user 影響なし。Step A landing 時点では builder が deprecated Artifact Manifest を生成していたため、v3 namespace への初回 publish は Step B' / B 後に持ち越し。

**Step B' — `LocalArtifactBuilder` の OCI Image Manifest 化** (§12.2 行 3)。**Landed in PR #869.**

`LocalArtifactBuilder` を `OciArtifactManifestBuilder` ベースから `OciImageManifestBuilder` ベースに切り替え、`artifactType` field と `application/vnd.oci.empty.v1+json` empty config descriptor で OMMX artifact を表現する (5.5)。同時に dead code 化する `LocalManifest::Artifact` variant、`OCI_ARTIFACT_MANIFEST_MEDIA_TYPE` 定数、`ensure_ommx_artifact_manifest` validator、`local_registry::import` 系の Artifact Manifest dispatch を撤去する。Local Registry の persisted manifest は Image Manifest 単形式になり、reader / writer / tests が単純化される。

生成する manifest は SDK v2 archive build (`ocipkg::OciArtifactBuilder::new` 経由) と byte-level に整合する shape を採用する (5.5): top-level の `mediaType` field は出力せず、各 layer descriptor は空でも `annotations` field を render し、empty config descriptor は `annotations` を持たない。registry に publish する際の Content-Type は transport が個別に付与する。残差は JSON field 順序 (v3 は `stable_json_bytes` で alphabetical sort、v2 SDK は struct 宣言順) のみで、これは reproducible digest のための意図的な前進。

`publish_artifact_manifest` の SQLite 側 blob 分類は manifest の `config().digest()` 一致を見て empty config blob のみ `BLOB_KIND_CONFIG`、他は `BLOB_KIND_BLOB` で記録する (OCI dir import path との整合)。

**Step B — SQLite → remote の native push 実装** (§12.2 行 5、行 10 publish 部分)。**進行中: PR #867 (Step B' landing 後に再開)。**

`LocalArtifact::push()` を SQLite + CAS から remote registry に直接 stream する。Python `ArtifactInner::Local::push` の "legacy disk dir 経由 fallback" を撤去。Step B 後に `ghcr.io/jij-inc/ommx/v3/{miplib2017,qplib}:*` を初回 publish (v3 dataset release event)。

Transport crate は **`oci-client`** (ORAS project, [oras-project/rust-oci-client](https://github.com/oras-project/rust-oci-client)) を採用する。選定理由:

- `oci-spec` 0.9 ベースで OMMX 既存 dep と互換。`oci_client::Reference` は `oci_spec::distribution::Reference` の re-export。
- Apache-2.0、ORAS sub-project として継続メンテナンス (last release 2026-03)。
- 旧 `ocipkg` が unmaintained であることに加え、表面化させていた GCAR (Google Cloud Artifact Registry) auth challenge 互換性問題 (Jij-Inc/ommx#606、`AuthChallenge::try_from` の JSON parse 失敗) も新 transport で解消見込み。

**async 戦略 (1):** `oci-client` は async-only (`tokio` + `reqwest`)。Step B では transport wrapper module が private `tokio::runtime::Runtime` を保持し、`block_on` で sync 境界を保つ。`LocalArtifact::push()` の public signature は sync のまま。

**async 戦略 (2):** 後続 milestone で async surface を段階的に外側に広げ、最終的に Python 側は [`pyo3-async-runtimes`](https://docs.rs/pyo3-async-runtimes/) 経由で `await` 可能にする。Step B 時点では SDK entry での runtime 構築は導入しない。

**Step B scope の境界:** `Artifact<OciArchive>::push` / `Artifact<OciDir>::push` (archive output / legacy dir 経由 push) は `ocipkg` ベースのまま据え置き、Step C で扱う。Step B では `LocalArtifact::push()` のみが新 transport を経由する。CLI (`ommx push <image>`) と Python (`Artifact.push()`) は同じ `LocalArtifact::push()` を共有するように両方更新する: CLI の `ImageNameOrPath::parse` は SQLite registry を先に問い合わせ、`Command::Push` の `Local` 分岐は `LocalArtifact::try_open` → 新 native push に流す (pre-v3 user の legacy disk dir のみ存在する path は fall-through として残す — Step C で除去)。

**credential 解決:** OMMX は credential store を自前で持たない。新 transport は `OMMX_BASIC_AUTH_*` env var (CI 用 explicit override) → `~/.docker/config.json` (+ credential helper、`docker_credential` クレート経由) → anonymous の 3 段で解決する。これにより `docker login` / `gcloud auth configure-docker` / `aws ecr get-login-password` が既に container ecosystem に対して surface している credential をそのまま流用できる。`oci-client` 自体は credential lookup を持たないので、`RegistryAuth::{Anonymous, Basic, Bearer}` のいずれかを materialize する責任は SDK 側にある。

これに伴い、v2 までの `ommx login` サブコマンド (ocipkg の `~/.ocipkg/config.json` に書き込んでいた) を本 PR で削除する。OMMX が credential store を所有しない設計と矛盾するため、`~/.ocipkg/config.json` を resolver の追加 tier として読む案は採用しない。`ommx login` ユーザーは `docker login <registry>` に移行する。

**auth e2e テスト:** `rust/ommx/tests/auth_e2e.rs` が `testcontainers` 経由で ephemeral `registry:2` (anonymous / htpasswd) を立ち上げ、resolver の各 tier (anonymous / docker config / env override / 部分 env override bail) と CLI dispatch + 否定ケース (auth 強制の sanity / 誤 credential 拒否) を実 push で検証する (8 シナリオ)。`#[ignore = "requires docker"]` + `#[serial]` で env mutation を逐次化、CI は `--include-ignored --test-threads=1` で全 8 件を走らせる。

**Step C — lazy auto-migration の legacy double-write 撤廃** (§12.2 行 6、行 4 Archive 半分)。**進行中。**

`Artifact.load(image)` / `ommx load` の auto-migration を "remote/archive → legacy disk OCI dir → SQLite" の 2-stage から "remote/archive → SQLite" 直結に変更する。`import_oci_archive` / `pull_image` は staging を `tempfile::TempDir` (registry root 配下) に切り替え、`registry.root().join(image_name.as_path())` への promote を行わない。`pull_image` は事前に SQLite の ref を resolve して network fetch を short-circuit する (v2 era の "skip if legacy dir exists" を canonical ref store ベースに置換)。CLI の `ImageNameOrPath::parse` は legacy dir の存在を fall-through として読み取らなくなり、SQLite ref のみで `Local`/`Remote` を分岐する; `ommx push <local>` / `ommx inspect <local>` / `ommx save <image> <output>` は `LocalArtifact::open` 経由になり、新たに追加した `LocalArtifact::save` (SQLite + CAS → `OciArchiveBuilder` 直結) を使う。pre-v3 user の legacy dir のみが存在する path は `bail_not_found_locally` が `ommx artifact import` への migration hint を返す。

Python 側は `ArtifactInner::Dir` variant を削除し `{Archive, Local}` 2-variant に集約。`Artifact.load_archive(dir_path)` は dir 入力に対し `local_registry::import_oci_dir` で SQLite に identity-preserving import して `LocalArtifact` を返す (file 入力は従来通り `Artifact<OciArchive>` を直接 open)。`BuilderInner::Dir` も内部的に `LocalArtifactBuilder` を保持しているだけだったので `BuilderInner::Local` にリネーム。`Artifact<OciArchive>::push` の新 transport 統一は本 step では扱わず、後続 milestone に持ち越す (`auth_from_env` ベースで稼働を維持)。

A / B' / B / C 後に残る ocipkg seam は `Builder<OciArchiveBuilder>` (archive 出力) と `import::{archive, remote}` 内の temp staging のみ。§12.2 行 1 (native streamer 置換) と行 2 (ocipkg 公開 surface 撤去) はその次の milestone series で、§12.4 で D → F → H として分解する。

### 12.4 ocipkg 依存撤去の milestone series (D → F → H)

Step C landing 後に残存している ocipkg surface は機能軸で 5 つに分かれる:

| カテゴリ | 使用箇所 | 役割 |
|---|---|---|
| A. Archive 入出力フォーマット | `Artifact<OciArchive>`, `ArchiveArtifactBuilder::build`, `OciArchiveBuilder` (`save.rs`) | `.ommx` (tar = oci-archive) reader/writer |
| B. OCI Image Layout dir 入出力 | `Artifact<OciDir>`, `OciDirBuilder` (import staging), `OciArtifact::from_oci_dir` | `import::{archive, remote}` の temp staging 用 |
| C. Remote push/pull (Archive 側) | `Artifact<OciArchive>::push`, `Artifact<OciDir>::push`, `Artifact<Remote>::pull_to` | ocipkg の `Remote` / `RemoteBuilder` + `auth_from_env` |
| D. `Image` / `ImageBuilder` trait | `Artifact<Base: Image>` の汎用化, `ocipkg::image::copy` 5 箇所 | ocipkg の format-agnostic abstraction |
| E. `ocipkg::ImageName` | 公開 API ほぼ全て (`LocalArtifact::open`, builder, CLI, Python) | image ref parser |

これを以下の 3 step で順に切る。各 step は独立 PR、step 間で SDK が壊れない状態を維持する。

**Step D — native remote transport を Archive 側にも展開** (§12.2 行 1 remote 半分、行 4 Archive 残り半分)。

`Artifact<OciArchive>::push` を Step B で導入した `oci-client` ベースの `remote_transport::RemoteTransport` に切り替え、`Artifact<Remote>::pull_to` を撤去して `pull_image` を remote → SQLite 直結にする。後者は manifest / config / layer blobs を `oci-client::Client::pull_blob` 経由で `FileBlobStore` に直接書き、SQLite の blob / manifest / ref レコードを 1 transaction で publish する。`Artifact<OciDir>::push` は CLI 経路を Step C で失った上 Python 側でも未使用なので Step D で削除する。

Step D の境界: archive / dir に対する `LocalArtifact::push()` 経路 (Step B で導入) は触らない。auth e2e テスト (`rust/ommx/tests/auth_e2e.rs`) は `Artifact<OciArchive>::push` を新 transport で経由するシナリオが追加され、`auth_from_env` ベースの単独 push 経路は撤去される。残る ocipkg `Remote` / `RemoteBuilder` / `auth_from_env` も同時に撤去する。

**Step F — native archive (tar) reader / writer** (§12.2 行 1 archive 半分、行 4 Archive 残り)。

`.ommx` archive は **tar of OCI Image Layout** に過ぎないので、`tar` crate 直叩きの native 実装に置換する。Writer は `LocalArtifact::save` を SQLite から blob を読みながら tar entry を append、最後に `index.json` を追記する形式に書き直す。Reader (`Artifact<OciArchive>` の `get_blob` / `get_manifest`) は 2 つの選択肢があり、

1. tar の random access reader を独自実装する
2. archive を開いた瞬間に `import_oci_archive` 同等の経路で temp SQLite registry に展開し、以降は `LocalArtifact` として読む (`load_archive(file)` を `load_archive(dir)` と同じ "import 経路" に揃える)

2 のほうが implementation cost が低く、本書の "SQLite が canonical store" の方針とも整合する。最終決定は Step F PR で。同じ PR で `import::archive::import_oci_archive` の OciDir staging を撤去し、tar から直接 `FileBlobStore` + SQLite に書く。

Step F が landed すると ocipkg の `OciArchive` / `OciArchiveBuilder` / `OciDir` / `OciDirBuilder` / `OciArtifact` / `Image` / `ImageBuilder` の使用箇所が全て消える。`Artifact<Base: Image>` の generic 化も意味を失うので、`Artifact<OciArchive>` は `OmmxArchive` のような単型 struct に折る (Python `ArtifactInner::Archive` も同名で更新)。

**Step H — `ImageName` 公開 surface 撤去** (§12.2 行 2)。

最後に残る ocipkg surface。最大の blast radius (`LocalArtifact::open` / `LocalArtifactBuilder::new` / annotation builder / CLI parse / Python `Artifact.load(...)` 全てが `ocipkg::ImageName` を expose) を持つので、Step D / F の後に独立 PR として進める。

候補は `oci_client::Reference` (= `oci_spec::distribution::Reference` の re-export、`oci-client` が既に dep にあるので zero-add) だが、`ocipkg::ImageName` の `as_path` / `hostname` / `port` / `name` / `reference` フィールドアクセスは SQLite ref store の repository key 構築 (`image_name_repository`) や CLI parse 経路で深く使われており、`oci_client::Reference` の API shape とは微妙にズレる。**独自 type `ommx::artifact::ImageRef` で wrap し、`oci_client::Reference` を内部に保持しつつ Display / FromStr / fields accessor を新 type 側で提供する** ほうが現実的な見込み。最終 type 設計は Step H PR で確定する。

Step H は公開 signature の breaking change を含むので、**v3.0 stable release 前に必ず land させる**。Python 側の `Artifact.load(image_name: str)` は `&str` を受け取って Rust 側で parse する形なので Python user 影響は限定的、影響は主に Rust SDK consumer に集中する。

Step H 完了で `ocipkg` を `Cargo.toml` から削除でき、v3 における ocipkg 依存撤去が完了する。

**順序の依存関係:** D (network) と F (local format) は独立で、どちらが先でも問題ない。H は D / F の両方が landed して `Artifact<Base: Image>` / `OciArchive` などの公開 type が消えてから着手する (callsite が新 type に対応している必要があるため)。順序は `D → F → H` または `F → D → H` のどちらでもよい。Step C と同程度の粒度感で、3 PR で完了見込み。
