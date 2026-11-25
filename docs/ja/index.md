# OMMXとは

OMMX (Open Mathematical prograMming eXchange) は数理最適化を実務に応用する上で必要になる問題の記述・変形・配布のための拡張可能な中間表現です。

多くの場合数理最適化ソフトウェアはソルバーとモデラーからなります:

- ソルバーは「与えられた最適化問題に対して解を与える」という中核的な操作を担当します
- モデラーは「業務上の課題を最適化問題として記述する」という作業自体が困難であるという点を解決するためのソフトウェアです

<!-- 実務上特化ソルバーを作る必要がある -->
一方、数理最適化の問題は単一のアルゴリズムでどんな問題でも解けるという構造にはなっていないため、
問題に応じて最適なアルゴリズムを選択あるいは個別に実装する必要があります。
それぞれのアルゴリズムへの適用限界は高速に自動で判定するのは通常困難で、
その問題自体の数理的な構造を頼りに専門家が複数の定式化を比較検討するというのが一般的です。
発展的なソルバーはアルゴリズムの選択を行いながら求解を行うことで汎用性をあげる方針をとりますが、どうしても特化した実装に対して性能面で劣ります。
なので実務的に数理最適化を業務に組み込む場合、具体的な問題固有とは行かないまでも問題の構造に特化したアルゴリズムの実装が必要になる場合が多くなります。

<!-- 個別ソルバーを作るにしても重複部分が多い -->
これをソフトウェアとしてみた時、特化が必要なのは主にコアの最適化アルゴリズムであり、
入出力の管理や問題の簡略化(Presolve)、あるいは入力された問題を特化アルゴリズムが解きやすい形に変換するなどの処理は部品単位で見ると多くは重複します。
またほとんどの特化アルゴリズムは内部的に既存のソルバーを利用するため、その際のAPIの標準化なども共通基盤として整備する価値があります。
問題構造に特化したソルバーでは最適化問題自体に加えて、例えばどの部分を先に解くか等の追加の情報を問題を記述したモデラー上で伝える必要あります。

<!-- なぜ中間表現を導入するのか -->
OMMXではソルバーとは独立して最適化問題を表現する拡張可能な中間表現を導入し、その上の変換としてこれらの処理を共通化・標準化していきます。
モデラーから中間表現を出力し、ソルバーは中間表現を受け取って解を中間表現に沿う形で与えます。
モデラーはソルバーの事情とは独立に「業務上の課題を最適化問題として記述する」という役割、より宣言的に「何を解くか」を明確にするという作業に集中でき、
ソルバーは特化実装を作るたびに同じような問題の簡略化や変形の再実装を避けられます。

```{toctree}
:caption: OMMXを使う目的と判断
:maxdepth: 1
:hidden:

book/transform_and_solve
book/evaluate_and_interpret
book/record_and_share
```

```{toctree}
:caption: 実行環境を用意する
:maxdepth: 1
:hidden:

getting_started/install
```

```{toctree}
:caption: 題材を通して試す
:maxdepth: 1
:hidden:

examples/tsp_sampling_with_openjij_adapter
```

```{toctree}
:caption: Artifactを調べて共有する
:maxdepth: 1
:hidden:

how_to/manage_local_registry
how_to/share_artifact_registry
```

```{toctree}
:caption: 外部ツールを接続する
:maxdepth: 1
:hidden:

how_to/agent_skills
```

```{toctree}
:caption: API・形式仕様を調べる
:maxdepth: 1
:hidden:

api/index
Rust API（公開版） <https://docs.rs/ommx/>
Message形式のスキーマ <https://github.com/Jij-Inc/ommx/tree/main/proto/ommx>
Artifact形式仕様 <https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md>
```

```{toctree}
:caption: SDKを移行する
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
```

```{toctree}
:caption: リリースごとの変更を確認する
:maxdepth: 1
:hidden:

release_note/ommx-3.0
release_note/ommx-2.5
release_note/ommx-2.4
release_note/ommx-2.3
release_note/ommx-2.2
release_note/ommx-2.1
release_note/ommx-2.0
release_note/ommx-1.9
release_note/ommx-1.8
release_note/ommx-1.7
release_note/ommx-1.6
release_note/ommx-1.5
```
