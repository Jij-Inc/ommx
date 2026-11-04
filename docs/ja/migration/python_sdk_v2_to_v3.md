(python-sdk-v2-to-v3-migration-guide)=
# Python SDK v2からv3への移行ガイド

このガイドでは [OMMX 2.9.4](https://pypi.org/project/ommx/2.9.4/) のコードを `main` のv3 APIに移行します。v2環境を残し、[導入手順](../getting_started/install.md)に従って別の環境にv3を用意してください。コードを変更する前に、代表的なモデルと候補を一つ選び、アプリケーションで使っている結果を保存します。{ref}`第6節 <migration-verify>`で移行後の結果と比較します。

**移行の最初に、手元のスクリプトを1本、型チェッカーにかけてください。** これまで使ったことがなくても、v3で書き換えが必要な呼び出しの多くをその場で見つけて直せます。まず{ref}`Pyrightを初めて実行する手順 <migration-first-type-check>`を試してください。指摘された旧API名は{ref}`巻末の索引 <migration-index>`から引けます。

```{contents} 移行ガイドの目次
:local:
:depth: 1
```

(migration-imports)=
## 1. SDKの読み込みとオブジェクトの扱いを変える

(migration-first-type-check)=
### 手元のスクリプトを型チェッカーにかける

v3をインストールしたプロジェクトのディレクトリで、次のコマンドを実行します。`solve.py` は、普段OMMXを使っているPythonスクリプトのパスに置き換えてください。uvがこの実行用にPyrightを用意します。型注釈や設定ファイルを追加する前に、今あるコードのままで試せます。

```sh
uv run --no-sync --with pyright pyright solve.py
```

PyrightはOMMXに同梱された型情報を使い、スクリプトを実行せずに、呼び出しが合わないファイルと行を示します。古い `ommx.v1` のimportが指摘されたら、まずそこを直すと、その先で使っているSDKオブジェクトの型も分かるようになります。たとえば `instance.write_mps(path)` が見つからないと表示されたら、`instance.save_mps(path)` に変えて同じコマンドを再実行します。残りの指摘を順に直したら、次のスクリプトに進んでください。

| Pyrightが指摘する箇所 | 書き換え方 |
|---|---|
| `Instance.from_components` の `constraints` にリストを渡している | 各制約の追加には {meth}`~ommx.Instance.add_constraint` を使う。 |
| `instance.write_mps` が見つからない | {meth}`~ommx.Instance.save_mps` を使う。 |
| `solution.constraints_df()` の引数が足りない | 通常制約なら `kind=ConstraintKind.Regular` のように種類を指定する。 |

型が同じでも意味が変わる場合があります。たとえば {meth}`~ommx.Solution.extract_constraints` で取得する値は引き続き浮動小数ですが、通常制約では違反量を表すようになりました。この値を使う帳票は、{ref}`同じ候補を両方のSDKで評価する例 <migration-compare-model>` に沿って照合してください。

### 公開APIのimportに切り替える

`Function`、`Constraint`、`DecisionVariable`、`Instance`、`ParametricInstance`、`Solution`、`SampleSet` は、2.9.4でも `ommx` から直接importできます。この7クラスはv2環境のうちにimport先を変更できます。v3では `Linear`、`Parameter`、`Equality`、`Sense` などのモデリング用の型も `ommx` からimportします。

`ommx.v1.*_pb2` や `.raw` を使っていた箇所は、何を取り出していたかで置き換え先が決まります。たとえば目的関数の向きは `instance.sense` で読めます。モデルは {meth}`~ommx.Instance.minimize` または {meth}`~ommx.Instance.maximize` で作り、変数と制約を順に追加します。バイト列を受け渡す場合は{ref}`形式に応じた読込み方法 <migration-storage>`を選んでください。protobufスキーマ名のv1は引き続きデータ形式を表します。v3のPython SDKでは生成済みprotobufモジュールと `from_raw`、`from_protobuf`、`to_protobuf` は廃止されました。

(migration-build)=
## 2. モデルを組み立て、編集する

(migration-automatic-ids)=
### モデルに採番を任せるか、外部のIDを引き継ぐか

v2でモデル構築のためだけに変数や制約に連番を振っていた場合は、v3では {class}`~ommx.Instance` に採番を任せられます。最初に最大化か最小化かを指定してモデルを作り、返された変数をそのまま式に使います。制約を追加したときの戻り値から、その制約に割り当てられたIDを取得できます。

```python
from ommx import Instance

model = Instance.maximize()
x = model.new_binary("x")
y = model.new_binary("y")
model.objective = x + y
limit = model.add_constraint(x + y <= 1, "limit")
assert model.constraints[limit.constraint_id].name == "limit"
```

整数・連続・半整数・半連続・有限集合を値域とする変数にも、それぞれ `new_integer`、`new_continuous`、`new_semi_integer`、`new_semi_continuous`、`new_finite_domain` があります。`Instance` と `ParametricInstance` では、`add_constraint`、`add_indicator_constraint`、`add_one_hot_constraint`、`add_sos1_constraint` が各制約の番号を割り当てます。

外部のアプリケーションがモデル内の変数0や制約7をその番号で参照する必要がある場合は、**その番号を維持します**。変数は作成時にIDを指定します。制約はv2の `constraints=[(x <= 1).set_id(7)]` を、{meth}`~ommx.Instance.from_components` に渡す `constraints={7: x <= 1}` に置き換えます。モデルに属していない制約はIDを持ちません。一つのモデルでは削除済みを含めて全種類の制約が番号を共有し、構築時や読込み時に重複を拒否します。

(42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)=
(typed-constraint-ids-and-live-collections)=
### 編集する行を選ぶ

通常制約7は `model.constraints[ConstraintID(7)]` で参照します。全種類を含むコレクションでは、検索キーに制約の種類を示す型付きIDが必要です。通常制約だけに絞った `model.constraints.of_kind(ConstraintKind.Regular)[7]` なら整数も使えます。反復すると型付きIDが得られます。IDと行の両方を使う場合は `.items()` を呼びます。

モデルから取得した有効な行は、モデルに接続されたハンドルです。`row.set_name("limit")` はモデル上の名前を更新します。独立したコピーが必要なら `row.detach()` を呼びます。v2の `add_name` と `add_description` は `set_name` と `set_description` に置き換えます。パラメータ全体の置換には `set_parameters`、指定したキーだけの更新には `add_parameters` を使います。変数の固定値は変数定義をコピーしてもモデル側に残るため、`model.fixed_decision_variables()` から取得します。詳しくは {class}`~ommx.ConstraintCollection` と {class}`~ommx.DecisionVariable` を参照してください。

(migration-constraint-metadata)=
### メタデータの編集先を確認する

```{index} Constraint.add_name (v2), Constraint.add_description (v2), Instance.get_constraint_by_id (v2), Instance.decision_variable_analysis (v2)
```

2.9.4では、`get_constraint_by_id` で取り出した制約の名前を変更しても、変更先は取り出したコピーだけでした。このコードを移行するときは、モデルの名前を更新したかったのか、表示用のコピーだけを編集したかったのかを確認してください。v3では次のように書き分けます。

```python
from ommx import Instance

model = Instance.minimize()
x = model.new_binary("x")
live = model.add_constraint(x <= 1, "original")
local = live.detach()
local.set_name("display")
assert model.constraints[live.constraint_id].name == "original"
live.set_name("capacity")
assert model.constraints[live.constraint_id].name == "capacity"
```

`dict(model.constraints)` で辞書を作っても、値は引き続きモデルに接続されています。独立させたい有効な行ごとに `detach()` を呼んでください。切り離した制約のsetterは呼出し元を変更してコピーを返すため、一つのオブジェクトに複数の変更を加えるときは、そのオブジェクトに対して個別に呼びます。

### 変数の役割を読む処理を置き換える

一変数の役割を判定するなら {meth}`~ommx.Instance.decision_variable_role`、複数の変数を帳票にまとめるなら `decision_variables_df()` の `state_role`・`substituted_value` 列を使います。Adapterでソルバーに渡す変数だけが必要なら `used_decision_variables` を使います。固定値や従属変数を候補の評価用に保持している場合、これらの問いの答えは異なります。

### 数式と制約の意味を引き継ぐ

- **制約ヒント**。v1のバイト列を読むと、通常制約は保持され、補助情報の `ConstraintHints` は無視されます。元のモデルでOneHotやSOS1が要件として定義されている場合は、対応する特殊制約をv3で直接作れます。ヒントだけを根拠に通常制約を取り除くと、許される解が変わることがあります。
- **式**。展開済みの多項式では係数を取得できます。複合式を含む {class}`~ommx.Function` では `degree()` と `num_terms()` が `None` を返すことがあります。二乗した多項式の係数が必要な場合は多項式同士を乗算してください。`Function` のべき乗は複合式を作ります。
- **パラメータ**。`Parameter(3, name="capacity")` で作り、`with_parameters` にはIDから値への辞書を渡します。{class}`~ommx.ParametricInstance` の `parameters` ではパラメータのオブジェクト、`parameters_df()` では表を取得できます。

整数IDを使う外部データがある場合は、どの種類のIDかも確認してください。変数IDとサンプルIDは整数のままです。制約IDは結果や由来の記録でも型付きで返り、`int()` で整数には変換できません。独自の変換では `ConstraintIDBase` と `ConstraintView` を共通型として扱い、既知の種類だけで網羅したと仮定しないでください。

(migration-solve)=
## 3. ソルバー・サンプラーに渡す

Adapterの推奨設定で準備する場合は、元のモデルを `solve()` または `sample()` に渡します。これらはコピーを準備します。変換方針を指定する場合は、変更してよいモデルに対して `instance.prepare(adapter.INPUT_CLASS, policy)` を呼び、準備した入力を `solve_without_preparation()` または `sample_without_preparation()` に渡します。各ソルバーの入力条件と実行例には[Adapter一覧](../api/adapters)から進めます。

**初期候補とペナルティ**。SCIPには2.9.4と同じく、変数名と添字で初期候補を渡せます。OpenJijの初期状態は、準備後にソルバーに渡す変数のIDで指定します。そのため、準備済みの入力を受け取るAPIに渡します。制約の変換に固定ペナルティが必要なら重みを明示してください。v2で直接渡していた重みは `policy.fixed_penalty`、整数のスラック変数の設定は `policy.integer_slack` に移します。一連の実行例は {class}`OpenJij AdapterのReference <ommx_openjij_adapter.OMMXOpenJijSAAdapter>` にあります。

**係数を取り出して解く場合**。{meth}`~ommx.Instance.to_qubo` と {meth}`~ommx.Instance.to_hubo` はモデルを変更し、ソルバーに渡すエネルギーを取り出せるようにします。v3では、このモデルで候補を評価すると準備前の目的関数の値を取得できます。ペナルティやソルバーのスコアを比較するアプリケーションでは、ソルバー側のエネルギーも別に保持してください。

(migration-openjij-penalties)=
### OpenJijに渡していたペナルティを準備方針に移す

```{index} sample_qubo_sa (v2), uniform_penalty_weight (v2), penalty_weights (v2), inequality_integer_slack_max_range (v2)
```

v2で `uniform_penalty_weight` を渡していた場合や暗黙の重み1に依存していた場合は、元の目的関数の大きさに合わせて重みを選びます。この例では、許される最良の選択に比べて両方を選ぶと目的値が2だけ増えるので、重み4でその利得を上回ります。重みを選び直せるよう、元のモデルをコピーしてから準備します。実行にはOpenJij Adapterが必要です。

```python
from copy import copy

from ommx import FixedPenaltyPreparation, Instance, Sense
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

source = Instance.maximize()
x, y = source.new_binary("x"), source.new_binary("y")
source.objective = 3 * x + 2 * y
source.add_constraint(x + y == 1)
prepared = copy(source)
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
)
prepared.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)
samples = OMMXOpenJijSAAdapter.sample_without_preparation(prepared, num_reads=2, seed=42)
assert samples.sense == Sense.Maximize
```

サンプリング結果では元の目的関数を読めます。また、双対値を利用するコードでは、準備済み入力に出力用の目的関数が明示されているとHiGHSとPython-MIPは双対値を付けない点も確認してください。

### サンプリング結果を受け取る側を更新する

v2の `sample_qubo_sa` が返したのは生のサンプルです。v3のAdapterは評価済みの {class}`~ommx.SampleSet` を返します。手元のOpenJijのレスポンスをデコードするだけのコードなら、`response_to_samples` を `decode_to_samples` に変更します。重みを変更するときは元モデルから再度準備してください。すでに入力条件を満たすモデルを準備し直しても、目的関数に組み込まれたペナルティは置き換わりません。

### 独自Adapterを実装している場合

受け取れる入力を `INPUT_CLASS` で宣言し、`solve_without_preparation()` または `sample_without_preparation()` を実装します。ソルバー固有のオプションは型付き引数として定義します。準備後の変数に対するオプションは、変換前後で対応付ける方法がなければ、準備済み入力のAPIで受け取ります。未対応の制約を読み飛ばすと元の要件に反する解が許されるため、その種類を明示的に拒否します。詳しくは {class}`~ommx.adapter.SolverAdapter`、{class}`~ommx.adapter.SamplerAdapter` を参照してください。

(migration-results)=
## 4. 候補を評価し、結果を取り出す

(migration-compare-model)=
(constraint-violation-migration)=
### 集計に必要なのは符号付きの値か、違反量か

候補 $x=0$ に対して、制約 $x-1\leq0$ の式の値は $-1$、違反量は $0$ です。候補がこの制約を満たすという判定は両方のSDKで同じです。変わったのは名前で取得する数値の意味です。

**v2.9.4**

```python
from ommx import DecisionVariable, Instance

x = DecisionVariable.binary(0)
limit = (x <= 1).set_id(7).add_name("limit")
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints=[limit], sense=Instance.MAXIMIZE,
)
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == -1.0
```

**v3（`main`）**

```python
from ommx import Instance

model = Instance.maximize()
x = model.new_binary("x")
model.objective = x
limit = model.add_constraint(x <= 1, "limit")
result = model.evaluate({x.id: 0})
assert result.extract_constraints("limit")[()] == 0.0
assert result.get_constraint_value(limit.constraint_id) == -1.0
```

{meth}`~ommx.Solution.extract_constraints` は全種類の制約を検索し、添字タプルをキーとして非負の違反量を返します。同じ名前に対して添字が重複すると、種類が違っていても曖昧になります。通常制約の符号や余裕を使っていた集計では、上の例のように `get_constraint_value` で式の値を読んでください。型付きIDで違反量を調べる場合は `constraint_violation` を使います。評価済みの行で `row.is_feasible(atol=result.feasibility_atol)` を呼ぶと、結果と同じ許容誤差で判定できます。候補全体の実行可能性には変数の値域も影響するため、制約違反量の合計がゼロでも実行不可能な場合があります。

通常制約の違反量は、等式なら残差の絶対値、不等式なら上限を超えた量です。特殊制約にも種類ごとの違反量があります。`total_violation()` は、保持された削除済み制約も含めて各違反量を足します。変換で行を追加した場合、この合計は変換前と一致するとは限りません。定義と許容誤差は {meth}`~ommx.Solution.total_violation` を参照してください。`evaluate()` で必要な変数が不足した場合の例外処理は `ValueError` に更新します。`partial_evaluate()` には一部の変数だけを渡せます。

### 表とサンプル集計を更新する

`*_df` はメソッドとして呼びます。たとえば `solution.constraints_df` は `solution.constraints_df(kind=ConstraintKind.Regular)` に置き換えます。制約表のインデックスには種類別のIDオブジェクトが入るため、整数をキーにしていた表結合も見直してください。モデルの表では `removed=True` で有効な行と保持された削除済みの行を両方取得します。結果には保持された削除済み制約も含まれ、参照すると評価時点の行のコピーを返します。制約のコンテキスト、パラメータ、由来、削除理由を読む表でも `kind=` が必要です。

複数サンプルでは `samples.summary` で目的値と全体の実行可能性を、`samples.constraints_df(kind=...)` で制約別の結果を取得します。`samples.sample_ids()` は集合を返します。リストを受け取るコードには `samples.sample_ids_list` を使ってください。使用例は {class}`~ommx.Solution` と {class}`~ommx.SampleSet` にあります。

(migration-constraint-tables)=
### 制約IDで結合する集計を移行する

```{index} Instance.constraints_df (v2), Instance.removed_constraints_df (v2), SampleSet.summary_with_constraints (v2)
```

v2では `instance.constraints_df` のインデックス `id` を、整数をキーとする業務側の表に結合できました。v3で通常制約のインデックス名は `regular_constraint_id`、値は `ConstraintID` です。モデルを組み立てるときに、業務側の制約番号（例では7）と追加時に返ったIDの対応を記録し、それを使って結合します。

```python
import pandas as pd
from ommx import ConstraintKind, Instance

model = Instance.maximize()
x = model.new_binary("x")
model.objective = x
limit = model.add_constraint(x <= 1, "limit")
id_map = {7: limit.constraint_id}

owners = pd.Series({7: "planning"}, name="owner")
rows = model.constraints_df(kind=ConstraintKind.Regular, include=[])
report = rows.join(owners.rename(index=id_map))
assert report.loc[limit.constraint_id, "owner"] == "planning"
```

ラベルとパラメータを列に含めるなら `include=["label", "parameters"]`、キーと値の組を1行ずつ結合するなら `constraint_parameters_df(kind=...)` を使います。変換履歴は `constraint_provenance_df(kind=...)` に由来のステップごとに記録されます。モデル側の `removed=True` は有効な行も含むので、削除済み制約だけの旧帳票を再現するには `removed_reason` が欠損していない行を選んでください。

### 許容誤差付近で実行可能性を照合する

判定では制約ごとの違反量に許容誤差を適用します。SOS1の値が `(1, 0.00006, 0.00006)` のとき、非ゼロ値を一つだけ残すには、合計 `0.00012` の変更が必要です。小さい二値はそれぞれ `0.0001` 未満でも、許容誤差 `0.0001` でこの制約は実行不可能です。帳票を照合するときは、評価時に保存された許容誤差を使い、同じ種類の制約を比較してください。問合せ時の許容誤差を変えても、離散値の正規化はやり直されません。定義は {meth}`~ommx.Solution.total_violation` にあります。

(migration-storage)=
## 5. 保存済みモデルとArtifactを引き継ぐ

### モデルや結果のバイト列を読む

SDKの版とprotobufの形式には別々の番号があります。**SDK 2.9.4** の `Instance.to_bytes()` が出力したバイト列は **v1形式** です。v3では `Instance.from_v1_bytes(data)` で読みます。`ParametricInstance`、`Solution`、`SampleSet` でも同じ命名です。新しいファイルには所有元の `to_v2_bytes()` と、対になる `from_v2_bytes()` を使います。変数・式・制約を所属情報とともに保存したい場合は、それを含むモデルまたは結果をシリアライズします。

2.9.4でもモデルと結果のバイト列に注釈が保存されます。v3では `title` などの専用プロパティでメタデータを、`add_user_annotation` で利用者定義のキーを設定するため、`annotations` の辞書に直接書き込んでいた箇所を更新してください。

出力を2.xのプログラムにも渡す場合は、形式に加えて受信側が扱える表現も確認します。たとえば2.9.4のスキーマには有限集合を値域とする変数がありますが、2.xのSDKはその変数を含むモデルの読込みを拒否します。また出力用の目的関数を明示したモデルは `to_v1_bytes()` では書き出せません。特殊制約や結果の実行可能性判定の許容誤差を保つにはv2形式を使います。

MPSを他のツールに渡す場合は `write_mps` を {meth}`~ommx.Instance.save_mps` に変更してください。ファイル形式に収まらない構造やメタデータも含めてモデルを復元するには、SDKのバイト列を保存します。

(13-artifact-api-archive-becomes-an-exchange-format)=
### `.ommx` アーカイブを開く・作る

| v2のコードの用途 | v3での移行先 |
|---|---|
| `Artifact.load_archive(path)` で開く | マニフェストとレイヤーの記述を調べるだけなら `Artifact.inspect_archive(path)`。レイヤーの中身を読むなら `Artifact.import_archive(path)` でLocal Registryに取り込む。 |
| `ArtifactBuilder.new_archive(path, name)` でファイルを作る | `ArtifactDraft.new(name)` で作成してレイヤーを追加し、コミット後に `artifact.save(path)` で書き出す。 |
| `new_archive_unnamed(path)` で無名のアーカイブを作る | `ArtifactDraft.new_anonymous()` を使う。コミットすると名前が自動で付くため、`image_name is None` を前提にした判定を見直す。 |
| `ommx push` でファイルを送る | `ommx load` で取り込み、イメージ名でpushする。 |

アーカイブのAPIは {class}`~ommx.artifact.Artifact` にあります。整理するときは `ommx prune-anonymous` で参照の削除候補を、`ommx gc` で到達できないblobを確認できます。どちらも削除を実行するには `--delete` が必要です。詳しくは[Local Registryの管理手順](../how_to/manage_local_registry.md)を参照してください。

(migration-artifact-archives)=
### アーカイブを作ってモデルを読み戻す

```{index} ArtifactBuilder.new_archive (v2), ArtifactBuilder.new_archive_unnamed (v2), Artifact.load_archive (v2)
```

v2のアーカイブ用builderは `build()` でファイルを作りました。v3の `commit()` はLocal Registryへの保存、`save()` はファイルへの書出しです。次の例はレジストリに匿名の記録を作り、新しいディレクトリに書き出します。

```python
from pathlib import Path
from tempfile import TemporaryDirectory

from ommx import Instance
from ommx.artifact import Artifact, ArtifactDraft

model = Instance.minimize()
model.title = "Archive migration"
draft = ArtifactDraft.new_anonymous()
draft.add_instance(model)
artifact = draft.commit()
with TemporaryDirectory() as directory:
    path = Path(directory) / "model.ommx"
    artifact.save(path)
    manifest = Artifact.inspect_archive(path)
    assert manifest.image_name == artifact.image_name
    restored = Artifact.import_archive(path).get_instance()
    assert restored.title == model.title
```

旧版の無名アーカイブにイメージ名の注釈がない場合、`import_archive` がローカルの匿名名を付けます。後続処理で名前を使うなら戻り値の `image_name` を読んでください。書出し時の `save` は既存ファイルを上書きしないので、新しい出力先を指定します。

### 別のSDKで作ったファイルを読む

Local Registryに書き込まずにアーカイブの内容を調べるには {meth}`~ommx.artifact.Artifact.inspect_archive` を使います。マニフェストから分かるのはレイヤーの記述までで、モデルを取り出すときは {meth}`~ommx.artifact.Artifact.import_archive` が必要です。古いアーカイブのレイヤー記述に注釈がある場合、protobuf側にも同じOMMXキーがあればそちらが優先されます。保存済みの試行を移行するときはタイトルや利用者定義の注釈も照合してください。

(migration-verify)=
## 6. 移行結果を確かめる

1. 同じ元モデルに対して同じ候補を評価し、目的関数の向き、目的値、実行可能性、判定に使った許容誤差を比較します。変換を行う場合も、元の要件を基準に結果を照合します。
2. アプリケーションが実際に使う値を比較します。たとえば第4節で符号付きの値を使っていた帳票は、移行後も $-1$ を読む必要があります。違反量を表示する帳票なら、同じ候補に対して $0$ が期待値です。サンプルを表で集計する場合はサンプルIDと結合キーも確認します。
3. ファイルや別プロセスにデータを渡す場合は、実際に2.9.4で保存したバイト列やアーカイブをv3で読み、外部ID・メタデータ・評価結果を一つずつ照合します。2.xの受信側も運用する場合は、v3からの出力をその環境で読めることも確かめます。

(migration-index)=
## 付録　旧API名から置き換え先を探す

個別の詳しい移行例は、サイトの {ref}`総索引 <genindex>` から旧API名でも引けます。

| 旧API・使用箇所 | v3の書き方 | 節 |
|---|---|---|
| `.raw` / `from_raw` / `from_protobuf` / `to_protobuf` | 例：{attr}`instance.sense <ommx.Instance.sense>` / {meth}`Instance.minimize <ommx.Instance.minimize>` | {ref}`1 <migration-imports>` |
| `add_name` / `add_description` | {meth}`set_name <ommx.Constraint.set_name>` / {meth}`set_description <ommx.Constraint.set_description>` | {ref}`migration-constraint-metadata` |
| `ArtifactArchive` / `ArtifactDir` / `ArtifactBuilder` | {class}`Artifact <ommx.artifact.Artifact>` / {class}`ArtifactDraft <ommx.artifact.ArtifactDraft>` | {ref}`migration-artifact-archives` |
| `Artifact.load_archive` | {meth}`Artifact.import_archive <ommx.artifact.Artifact.import_archive>` / {meth}`Artifact.inspect_archive <ommx.artifact.Artifact.inspect_archive>` | {ref}`migration-artifact-archives` |
| `Constraint.id` / `set_id` / `next_constraint_id` | {meth}`add_constraint <ommx.Instance.add_constraint>`。外部の制約番号を維持するときは {meth}`from_components <ommx.Instance.from_components>` | {ref}`2 <migration-build>` |
| `constraint_hints` / `OneHot` / `Sos1` | {class}`OneHotConstraint <ommx.OneHotConstraint>` / {class}`Sos1Constraint <ommx.Sos1Constraint>` | {ref}`2 <migration-build>` |
| `constraints_df` / `removed_constraints_df` | {meth}`constraints_df(kind=...) <ommx.Instance.constraints_df>` / {meth}`constraints_df(kind=..., removed=True) <ommx.Instance.constraints_df>` | {ref}`migration-constraint-tables` |
| `decision_variable_analysis()` | {meth}`decision_variable_roles() <ommx.Instance.decision_variable_roles>` / {meth}`fixed_decision_variables() <ommx.Instance.fixed_decision_variables>` | {ref}`migration-constraint-metadata` |
| `DecisionVariable.substituted_value` | {meth}`model.fixed_decision_variables() <ommx.Instance.fixed_decision_variables>` | {ref}`2 <migration-build>` |
| `evaluated.feasible` / `sampled.feasible` | {meth}`evaluated.is_feasible(atol=...) <ommx.EvaluatedConstraint.is_feasible>` / {meth}`sampled.feasible(atol=...) <ommx.SampledConstraint.feasible>` | {ref}`4 <migration-results>` |
| `extract_constraints(name)` | 違反量は {meth}`extract_constraints <ommx.Solution.extract_constraints>`、通常制約の式の値は {meth}`get_constraint_value <ommx.Solution.get_constraint_value>` | {ref}`4 <migration-results>` |
| `get_constraint_by_id` / `get_removed_constraint_by_id` | {attr}`constraints[ConstraintID(n)] <ommx.Instance.constraints>` / {attr}`removed_constraints[ConstraintID(n)] <ommx.Instance.removed_constraints>` | {ref}`2 <migration-build>` |
| `Linear.from_object` / `Linear.equals_to` | 式または {meth}`Linear.single_term <ommx.Linear.single_term>` / {meth}`almost_equal <ommx.Linear.almost_equal>` | {ref}`2 <migration-build>` |
| `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` | {meth}`Linear.terms() <ommx.Linear.terms>` / {meth}`Quadratic.terms() <ommx.Quadratic.terms>` / {meth}`Polynomial.terms() <ommx.Polynomial.terms>` | {ref}`2 <migration-build>` |
| `ommx.v1` / `ommx.v1.*_pb2` | {doc}`ommx </api/index>` | {ref}`1 <migration-imports>` |
| `Parameter.new` / `Parameters(entries=...)` | {class}`Parameter(id, ...) <ommx.Parameter>` / {meth}`with_parameters({id: value}) <ommx.ParametricInstance.with_parameters>` | {ref}`2 <migration-build>` |
| `ParametricInstance.parameters (DataFrame)` | {meth}`parameters_df() <ommx.ParametricInstance.parameters_df>` | {ref}`2 <migration-build>` |
| `response_to_samples` / `sample_qubo_sa` | {func}`decode_to_samples <ommx_openjij_adapter.decode_to_samples>` / {meth}`OMMXOpenJijSAAdapter.sample <ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample>` | {ref}`migration-openjij-penalties` |
| `SampleSet.sample_ids` / `summary_with_constraints` | {meth}`sample_ids() <ommx.SampleSet.sample_ids>` / {attr}`summary <ommx.SampleSet.summary>` + {meth}`constraints_df(kind=...) <ommx.SampleSet.constraints_df>` | {ref}`4 <migration-results>` |
| `Solution.total_violation_l1` / `total_violation_l2` | {meth}`total_violation <ommx.Solution.total_violation>`（各制約の違反量の和）。L2集計は廃止 | {ref}`4 <migration-results>` |
| `to_bytes` / `from_bytes` | {meth}`to_v2_bytes <ommx.Instance.to_v2_bytes>` / {meth}`from_v2_bytes <ommx.Instance.from_v2_bytes>` / {meth}`from_v1_bytes <ommx.Instance.from_v1_bytes>` | {ref}`5 <migration-storage>` |
| `uniform_penalty_weight` / `penalty_weights` / `inequality_integer_slack_max_range` | {attr}`policy.fixed_penalty <ommx.PreparationPolicy.fixed_penalty>` / {attr}`policy.integer_slack <ommx.PreparationPolicy.integer_slack>` | {ref}`migration-openjij-penalties` |
| `used_decision_variable_ids() (Instance, Function)` | {meth}`Instance.required_ids() <ommx.Instance.required_ids>` / {meth}`Function.required_ids() <ommx.Function.required_ids>` | {ref}`2 <migration-build>` |
| `write_mps(path)` | {meth}`save_mps(path) <ommx.Instance.save_mps>` | {ref}`5 <migration-storage>` |

v3の各プレリリースでの変更経緯は[Python SDK 3.0のリリースノート](../release_note/ommx-3.0.md)を参照してください。
