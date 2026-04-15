# OMMX Python SDK 2.0.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.0)

2024/7/10の [OMMX Python SDK 1.0.0](https://github.com/Jij-Inc/ommx/releases/tag/python-1.0.0) リリースから約1年ぶりのメジャーバージョンのリリースです。このバージョンでは大幅な性能向上に加え、破壊的な変更を含むAPIの改善、さらに新機能の追加が行われています。

```{note}
OMMXではSDKのバージョンとデータフォーマットのバージョンは独立しています。新しいSDKでも既存のデータは全て読み込むことができます。
```

## 性能向上

OMMXの初期設計では標準化されたデータ形式を提供することが主目的だったため、SDKにおけるモデルの生成のAPIは主にテストとデバッグ用途であり、性能はあまり重視されていませんでした。しかし、OMMXレベルでQUBOへの変換などの機能が実現されるようになり、性能のボトルネックになることが増えてきました。

このバージョンでは、OMMXのAPIの性能を大幅に向上させました。計算量オーダーレベルで改善されているところが多々あるため、特に大規模な問題においては大幅な性能向上が期待できます。特に、以下の点で改善が行われています。

- Python用にProtocol Buffersのスキーマ定義から自動生成されていたAPIの実装を、Rust SDKをベースにした実装に置き換えました。これにより不必要なシリアライズとデシリアライズのオーバーヘッドが削減され、APIの呼び出しが高速化されました。
- Rust SDKにおいてもスキーマ定義から自動生成されていた部分をよりRustとして自然に実装し直しました。より適切なデータ構造を利用できるようになったため大幅な性能向上が実現されています。また整合性の検査、例えば決定変数として登録されていない変数を含んだ多項式を目的関数として登録できないといったProtocol Buffersでは記述できなかった制約を、Rustの型レベルで保証できるようになりより効率的で厳密に検査できるようになりました。
- [CodSpeed](https://codspeed.io/Jij-Inc/ommx) によるRustおよびPython SDKのオンラインプロファイリング・継続的ベンチマーキング環境を整えました。このリリースでも大幅な改善を行いましたが、まだまだ最善からは遠い箇所が多々あり、今後も継続的に改善を行っていきます。

## APIの更新

上述の通り、Protocol Buffersの定義から自動生成されていたAPIの置き換えに加え、[GitHub Copilot]や[Claude Code]等のAIアシスタントの普及に伴い、より自然でAIが生成しやすいAPIに改善しました。今回はメジャーバージョンアップのため破壊的な変更を含むAPIの改善を行っています。

特に[Claude Code]での利用を想定したマイグレーションガイドを [Python SDK v1 to v2 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md)にまとめてあります。[Claude Code]にこれを読み込ませた上でマイグレーションを行うとよりスムーズに移行できるでしょう。なお `pyright` や `mypy` による型チェックを併用するとさらにスムーズに移行できます。

[GitHub Copilot]: https://github.com/features/copilot
[Claude Code]: https://www.anthropic.com/claude-code
[`ommx.v1.Instance`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance
[`ommx.v1.ParametricInstance`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.ParametricInstance
[`ommx.v1.Solution`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Solution
[`ommx.v1.SampleSet`]: https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.SampleSet
[`DataFrame`]: https://pandas.pydata.org/pandas-docs/stable/reference/frame.html

### `raw` APIの非推奨化

2.0.0より前では `ommx.v1.Instance.raw` などの `raw` フィールドはProtocol Buffersから自動生成されたclassを持つフィールドでしたが、上述の通りこれはRust SDKをベースにした実装に置き換わりました。今回このレイヤーにおける互換性は維持せず、代わりに [`ommx.v1.Instance`] のAPIを直接利用することで必要な処理が実現できるようになりました。今後段階的に `raw` APIを廃止していきます。

### DataFrameを返す関数API名称の変更

従来の `Instance.decision_variables` や `Instance.constraints` などのプロパティは [`DataFrame`] を返すものでしたが、これらは[`DataFrame`]を返すことを明確にするため [`Instance.decision_variables_df`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables_df) や [`Instance.constraints_df`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints_df) などの名称に変更されました。

代わりに [`Instance.decision_variables`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.decision_variables) や [`Instance.constraints`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.constraints) などのプロパティは、`list[ommx.v1.DecisionVariable]` や `list[ommx.v1.Constraint]` を返すようになりました。これらは決定変数および制約条件のIDでソートされています。これらは通常のPythonのコードとして利用するときに[`DataFrame`]を返すよりも自然に扱えるようになっています。IDから決定変数や制約条件を取得するには [`Instance.get_decision_variable_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_decision_variable_by_id) や [`Instance.get_constraint_by_id`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.get_constraint_by_id) を利用してください。

これらの変更は [`ommx.v1.ParametricInstance`] および [`ommx.v1.Solution`] や [`ommx.v1.SampleSet`] などのクラスでも同様です。

## 新機能

今回のリリースでは内部構造の変更とAPIの破壊的な変更を確定させることが主目的でしたが、いくつかの新機能も追加されています。

### OpenJijアダプターにおける HUBO (high-order unconstrained binary optimization) のサポート

OpenJijは3次以上の高次の多項式を目的関数として、次数下げなどの処理を行うことなく直接高速に扱うことができますが、これをOMMX Adapter経由で直接扱えるようになりました。合わせて [`ommx.v1.Instance`] に [`to_qubo`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.to_qubo) と同じように整数変数のバイナリエンコーディングや不等式制約の等式制約化を自動で行う [`to_hubo`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.to_hubo) メソッドが追加されています。

```{warning}
元々 `Instance` には `as_pubo_format` というメソッドがありましたが、2.0.0において [`as_hubo_format`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Instance.as_hubo_format) に名前が変更され戻り値も変更されています。PUBO (polynomial unconstrained binary optimization) と HUBO (high-order unconstrained binary optimization) は多くの場合、QUBO (Quadratic Unconstrained Binary Optimization) に対して3次以上の高次項も扱えるという意図でほぼ同じ意味で使われていますが、OMMXプロジェクトでは今後HUBOの名称を使用することにしました。
```

### Linux向けARM CPUサポート

Linuxのaarch64向けのバイナリパッケージ(wheel)が提供されるようになりました。これにより以下の環境などでより簡単にOMMXを利用できるようになりました。

- macOS上でのDocker等のLinux VM上での利用
- AWS GravitonやAmpereなどの高性能ARM CPUを利用したIaaS及びそれに対応したPaaS
- GitHub Actionsの `ubuntu-24.04-arm` 環境

---------

# OMMX Python SDK 2.0.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.1)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.2-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.2)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.3-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.3)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.4-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.4)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.5-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.5)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.6-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.6)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.7-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.7)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.8-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.8)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.9-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.9)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.10-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.10)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.11-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.11)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.0.12-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.0.12)

2.0.1以降のリリースについてはこちらでまとめます。詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。

## 新機能

### `ParametricInstance` のバリデーション強化 (2.0.3, [#566](https://github.com/Jij-Inc/ommx/pull/566))

`ParametricInstance` のデータ読み込み時のバリデーションが厳密になりました。未定義の変数を参照するなど意味的に不正なインスタンスをパース時に明確なエラーメッセージとともに拒否するようになりました。

### `Instance.used_decision_variables` と `penalty_method` (2.0.3, [#572](https://github.com/Jij-Inc/ommx/pull/572), [#553](https://github.com/Jij-Inc/ommx/pull/553))

`Instance.used_decision_variables` は目的関数や制約条件で実際に参照されている決定変数の集合を返します。`Instance.penalty_method` により制約付き問題から無制約ペナルティ定式化への変換ができます。

### MPS形式での2次目的関数・制約のサポート (2.0.5, [#597](https://github.com/Jij-Inc/ommx/pull/597))

MPSパーサーとライターが `QUADOBJ` および `QCMATRIX` セクション（Gurobiなどのソルバーで使用）に対応し、2次計画問題のMPS形式での完全なラウンドトリップが可能になりました。

### `ConstraintHints` の部分評価 (2.0.6, [#609](https://github.com/Jij-Inc/ommx/pull/609))

`Instance.partial_evaluate` 呼び出し時に、`OneHot` および `SOS1` の制約ヒントが固定された値を従属変数に自動的に伝播するようになりました。例えば one-hot 制約 `x0 + x1 + x2 = 1` に対して `x0=1` と固定すると、`x1=0` および `x2=0` が自動的に伝播されます。

### デフォルト絶対許容誤差の設定 (2.0.6, [#610](https://github.com/Jij-Inc/ommx/pull/610))

実行可能性チェックのグローバルな絶対許容誤差を、`ommx.set_default_atol()` / `ommx.get_default_atol()` または環境変数 `OMMX_DEFAULT_ATOL` で実行時に設定できるようになりました。デフォルト値は `1e-6` のままです。

### QPLIB を OMMX Artifact として配布 (2.0.9, [#640](https://github.com/Jij-Inc/ommx/pull/640))

453個のQPLIBベンチマークインスタンスを OMMX Artifact としてパッケージ化し、既存のMIPLIB2017サポートと同様に利用可能にしました。Pythonでは `ommx.dataset.qplib(tag)` および `ommx.dataset.qplib_instance_annotations()` でアクセスできます。

### `Instance.stats()` (2.0.11, [#652](https://github.com/Jij-Inc/ommx/pull/652))

インスタンスの階層的な統計情報を返す `Instance.stats()` メソッドを追加しました。決定変数の種類別（バイナリ/整数/連続/半連続/半整数）および用途別（目的関数、制約、固定、従属、無関係）のカウント、アクティブ・削除済み制約のカウントを含みます。

### Artifactレジストリ関数のPython公開 (2.0.7–2.0.12, [#622](https://github.com/Jij-Inc/ommx/pull/622), [#623](https://github.com/Jij-Inc/ommx/pull/623), [#625](https://github.com/Jij-Inc/ommx/pull/625), [#662](https://github.com/Jij-Inc/ommx/pull/662))

ローカルレジストリ管理関数（`get_local_registry_root`, `set_local_registry_root`, `get_image_dir`, `get_images`）を段階的にPythonに公開し、`ommx.artifact.*` にエイリアスを追加しました。すべて `pathlib.Path` オブジェクトを返します。

## バグ修正

### 最大化時にOpenJijの目的関数の符号が反転 (2.0.5, [#600](https://github.com/Jij-Inc/ommx/pull/600))

最大化問題を解く際、OpenJijアダプターは最小化問題に変換するために目的関数を符号反転していましたが、解を記録する前に戻していなかったため、目的関数の値の符号が反転していました。

### 制約緩和後に `ConstraintHints` の復元が失敗 (2.0.2, [#551](https://github.com/Jij-Inc/ommx/pull/551))

制約を緩和した後に元に戻すと、`ConstraintHints` が不整合な状態になる問題を修正しました。
