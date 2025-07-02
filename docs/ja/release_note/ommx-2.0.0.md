# OMMX Python SDK 2.0.0

2024/7/10の [OMMX Python SDK 1.0.0](https://github.com/Jij-Inc/ommx/releases/tag/python-1.0.0) リリースから約1年ぶりのメジャーバージョンのリリースです。このバージョンでは大幅な性能向上に加え、破壊的な変更を含むAPIの改善、さらに新がが機能の追加が行われています。

```{note}
OMMXではSDKのバージョンとデータフォーマットのバージョンは独立しています。新しいSDKでも既存のデータは全て読み込むことができます。
```

## 性能向上

OMMXの初期設計では標準化されたデータ形式を提供することが主目的だったため、SDKにおけるモデルの生成のAPIは主にテストとデバッグ用途であり、性能はあまり重視されていませんでした。しかし、OMMXレベルでQUBOへの変換などの機能が実現されるようになり、性能のボトルネックになることが増えてきました。

このバージョンでは、OMMXのAPIの性能を大幅に向上させました。計算量オーダーレベルで改善されているところが多々あるため、特に大規模な問題においては大幅な性能向上が期待できます。特に、以下の点で改善が行われています。

- Python用にProtocol Buffersの定義から自動生成されていたAPIの実装を、Rust SDKをベースにした実装に置き換えました。これにより不必要なシリアライズとデシリアライズのオーバーヘッドが削減され、APIの呼び出しが高速化されました。
- Rust SDKにおいてもProtocol Buffersの定義から自動生成されていた部分をよりRustとして自然に実装し直しました。より適切なデータ構造を利用できるようになったため大幅な性能向上が実現されています。また整合性の検査、例えば決定変数として登録されていない変数を含んだ多項式を目的関数として登録できないといったProtocol Buffersでは記述できなかった制約を、Rustの型レベルで保証できるようになりより効率的で厳密に検査できるようになりました。
- [CodSpeed](https://codspeed.io/) によるRustおよびPython SDKのオンラインプロファイリング・継続的ベンチマーキング環境を整えました。このリリースでも大幅な改善を行いましたが、まだまだ最善からは遠い箇所が多々あり、今後も継続的に改善を行っていきます。

## APIの更新

### DataFrameを返す関数API名称の変更

## 新機能

### OpenJijアダプターにおける HUBO (high-order unconstrained binary optimization) のサポート

OpenJijは3次以上の高次の多項式を目的関数として、次数下げなどの処理を行うことなく直接扱うことができますが、これをOMMX Adapter経由で直接扱えるようになりました。合わせて `ommx.v1.Instance` に `to_qubo` と同じように `to_hubo` メソッドが追加されています。

```{warning}
元々 `Instance` には `as_pubo_format` というメソッドがありましたが、2.0.0において `as_hubo_format` に名前が変更されました。PUBO (polynomial unconstrained binary optimization) と HUBO (high-order unconstrained binary optimization) は多くの場合、Quadraticで無いものも扱えるという意図でほぼ同じ意味で使われていますが、OMMXプロジェクトでは今後HUBOの名称を使用することにしました。
```

### ランダム生成
