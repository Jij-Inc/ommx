# OMMX Python SDK 2.9.x

## Unreleased

### Instanceのスナップショット読み取りAPI ([#1244](https://github.com/Jij-Inc/ommx/pull/1244))

次の2.xリリースに、v3開発版と共通で使える2つのメソッドを追加します。

- `Instance.fixed_decision_variables() -> dict[int, float]` は、明示的に保持された
  代入値を変数IDと対応付けて返します。上下界が一致するだけの変数は含みません。
- `Instance.regular_constraint_ids() -> set[int]` は、有効な通常制約のIDを返します。
  除去済み制約は含みません。OneHot/SOS1 hintは元の通常制約を除去しないため、
  そのIDは含まれます。

```python
fixed = instance.fixed_decision_variables()
constraints = {
    cid: instance.get_constraint_by_id(cid)
    for cid in instance.regular_constraint_ids()
}
```

どちらも独立したスナップショットを返し、該当する要素がなければ空の辞書・集合を返します。
読み取りや返却値の変更によってInstanceが変わることはありません。
これらは公開済みのSDK 2.9.0には含まれない追加APIです。

## バグ修正

### HiGHS のモデル構築を高速化 ([#1239](https://github.com/Jij-Inc/ommx/pull/1239))

HiGHS Adapter で項数の多い線形目的関数や制約を変換する際、加算のたびに
蓄積済みの式全体をコピーしないようにしました。
`OMMXHighsAdapter(instance)` はモデル構築のみを行い、制約追加前に求解を
実行しなくなります。変換から求解・解の復元まで行う場合は
`OMMXHighsAdapter.solve(instance)` を、構築済みモデルを求解する場合は
`adapter.solver_input.run()` を使ってください。
定数目的関数についても、定数値と最小化・最大化の方向を HiGHS モデルに保持します。

## 新機能

### Rust SDK v3からV1 Bridgeでモデルを受信 ([#1226](https://github.com/Jij-Inc/ommx/pull/1226))

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
