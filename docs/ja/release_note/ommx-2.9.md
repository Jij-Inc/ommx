# OMMX Python SDK 2.9.x

## バグ修正

### protobufシリアライズ時のアノテーション消失を修正 ([#1262](https://github.com/Jij-Inc/ommx/pull/1262))

`Instance`、`ParametricInstance`、`Solution`、`SampleSet` のユーザーアノテーションと
メタデータが、`to_bytes()` / `from_bytes()` の往復で保持されるようになりました。
Instanceのタイトル、説明、作成ツール名、ライセンス、データセット、著者、作成日時はprotobufのDescriptionに、
求解・サンプリングの来歴はProcessMetadataに保存されます。
既存の変更可能な `annotations` 辞書とアノテーション用メソッドは引き続き使えます。
`authors` プロパティは、カンマを含む名前や空の名前を含めてprotobufの各要素を保持し、
アノテーション辞書への編集も反映します。

```python
from ommx import Instance

instance = Instance.empty()
instance.title = "My model"
instance.add_user_annotation("source", "python")
restored = Instance.from_bytes(instance.to_bytes())
assert restored.title == "My model"
assert restored.get_user_annotation("source") == "python"
```

モデル変換、部分評価、パラメータの具体化、サンプルからSolutionへの取り出しでも
アノテーションを保持します。Artifactの読み込みでは、旧形式のdescriptorにのみ保存された
アノテーションを補完し、同じキーが両方にある場合はprotobuf側の値を優先します。
optionalな文字列フィールドに明示的な空文字がある場合も、protobuf側の値を優先します。
変数数と制約数は引き続きArtifactのdescriptorアノテーションとして扱います。

独自のアノテーションにはユーザー用または第三者の名前空間を使ってください。
`org.ommx.v1.*` はOMMXのメタデータ用に予約されており、この名前空間の未知のキーは
シリアライズ時に拒否されます。追加したprotobufフィールドはSDK v3と同じフィールド番号を使い、
`format_version = 0` を維持します。旧SDKでも数理モデルは読み込めますが、デコードして
再エンコードすると追加のメタデータが消失する場合があります。

Rustのモデル型には、拡張アノテーションの予約キーを検出するとエラーを返す
`try_to_bytes()` を追加しました。既存の `to_bytes()` は戻り値の `Vec<u8>` を維持し、
そのような不正なキーがある場合はpanicします。

`ommx.v1` のprotobuf定義は、`Function.Expression` の定義を含めてSDK v3と共通です。
SDK 2.xではこの関数表現は引き続き未対応であり、V1 Bridgeを含めてRust側の関数・モデルへ
変換する時点で拒否します。`ParametricInstance.from_bytes()` は従来どおり検証を遅延します。

### penalty構築前の整数エンコードと一括置換 (2.9.2, [#1254](https://github.com/Jij-Inc/ommx/pull/1254))

`Instance.log_encode()` は、指定された整数変数をまとめてエンコードします。
すべての変数のboundを検証してからInstanceを書き換えます。非循環の置換は、
置換同士の依存関係を先に解決し、各多項式の項を一度走査して処理します。

`to_qubo()` と `to_hubo()` は、slack変換の後、制約を二乗してpenaltyを構築する
前に整数エンコードを実行します。penaltyによって除去された制約には
エンコード後の式が保存されますが、制約値と実行可能性は引き続き復元された
決定変数を使って評価されます。別の順序で変換する場合は、個々の変換メソッドを
明示的に呼び出してください。

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
