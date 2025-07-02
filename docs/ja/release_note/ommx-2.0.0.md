# OMMX Python SDK 2.0.0

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
- AWS GravitonやAmpareなどの高性能ARM CPUを利用したIaaS及びそれに対応したPaaS
- GitHub Actionsの `ubuntu-24.04-arm` 環境
