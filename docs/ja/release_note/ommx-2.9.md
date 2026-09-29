# OMMX Python SDK 2.9.x

## バグ修正

### HiGHS のモデル構築を高速化 (2.9.0, [#1239](https://github.com/Jij-Inc/ommx/pull/1239))

HiGHS Adapter で項数の多い線形目的関数や制約を変換する際、加算のたびに
蓄積済みの式全体をコピーしないようにしました。
`OMMXHighsAdapter(instance)` はモデル構築のみを行い、制約追加前に求解を
実行しなくなります。変換から求解・解の復元まで行う場合は
`OMMXHighsAdapter.solve(instance)` を、構築済みモデルを求解する場合は
`adapter.solver_input.run()` を使ってください。
定数目的関数についても、定数値と最小化・最大化の方向を HiGHS モデルに保持します。

## 新機能

### Rust SDK v3からV1 Bridgeでモデルを受信 (2.9.0, [#1226](https://github.com/Jij-Inc/ommx/pull/1226))

Rust側でProtobufV1を選択し、既存のPython SDK 2.xのクラスへモデルを
転送できるようになりました。通常制約による定式化と、従来のソルバー
アダプター向けの`ConstraintHints`を保持します。受信側SDKがデータを検証し、
未対応の関数表現はエラーにします。

Bridgeが扱う7型をトップレベルからもインポートできます。
`ommx.Function`、`Constraint`、`DecisionVariable`、`Instance`、
`ParametricInstance`、`Solution`、`SampleSet`は、それぞれ`ommx.v1`の
同名クラスと同じクラスです。既存のインポートやソルバーAPIは引き続き使えます。
Bridgeのプロトコル・データ検証の失敗は`ommx.BridgeError`で通知します。

このSDKが受信できるのはProtobufV1です。送信側が転送前に数学的な表現を選び、
Bridge自体は制約のloweringや昇格を行いません。

## 改善

### ソルバーアダプターの Python バージョン上限を撤廃 (2.9.1, [#1246](https://github.com/Jij-Inc/ommx/pull/1246))

HiGHS、PySCIPOpt、OpenJij の各アダプターの Python 要件を
`requires-python = ">=3.10"` に変更し、従来の `<3.14` という上限を撤廃しました。
アダプター自身のパッケージメタデータによって Python 3.14 へのインストールが
制限されなくなります。インストールには引き続き、使用する Python バージョンと
プラットフォームに対応したソルバーおよび依存パッケージが必要です。
