---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# 様々なデータをOMMX Artifact形式で共有する

数理最適化のワークフローでは、多様なデータの生成と管理が不可欠です。これらのデータを適切に管理することで、計算結果の再現性が確保され、チーム内での効率的な共有が可能になります。

OMMXは、これらの多様なデータを効率的かつシンプルに管理する仕組みを提供します。具体的には、OMMX Artifactというデータ形式を定義し、最適化計算に関連する多様なデータの保存・管理・共有をOMMX SDKによって可能にします。

+++

## 事前準備：共有するデータ

まず共有するべきデータを用意しましょう。ナップザック問題を表す `ommx.v1.Instance` を作成し、SCIPによる最適化計算を行います。さらに最適化計算に対する分析結果も共有します。今回はこれらの処理の詳細は本題から離れるので省略します。

```{code-cell} ipython3
:tags: [hide-input]

from ommx.v1 import Instance, DecisionVariable, Constraint
from ommx_pyscipopt_adapter.adapter import OMMXPySCIPOptAdapter
import pandas as pd

# 0-1ナップサック問題のデータを用意する
data = {
    # 各アイテムの価値
    "v": [10, 13, 18, 31, 7, 15],
    # 各アイテムの重さ
    "w": [11, 15, 20, 35, 10, 33],
    # ナップサックの耐荷重
    "W": 47,
    # アイテムの総数
    "N": 6,
}

# 決定変数を定義する
x = [
    # バイナリ変数 x_i を定義する
    DecisionVariable.binary(
        # 決定変数のIDを指定する
        id=i,
        # 決定変数の名前を指定する
        name="x",
        # 決定変数の添え字を指定する
        subscripts=[i],
    )
    # バイナリ変数を num_items 個だけ用意する
    for i in range(data["N"])
]

# 目的関数を定義する
objective = sum(data["v"][i] * x[i] for i in range(data["N"]))

# 制約条件を定義する
constraint = Constraint(
    # 制約条件の名前
    name = "重量制限",
    # 制約式の左辺を指定する
    function=sum(data["w"][i] * x[i] for i in range(data["N"])) - data["W"],
    # 等式制約 (==0) or 不等式制約 (<=0) を指定する
    equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
)

# インスタンスを作成する
instance = Instance.from_components(
    # インスタンスに含まれる全ての決定変数を登録する
    decision_variables=x,
    # 目的関数を登録する
    objective=objective,
    # 全ての制約条件を登録する (キーは制約ID)
    constraints={0: constraint},
    # 最大化問題であることを指定する
    sense=Instance.MAXIMIZE,
)

# SCIPで解く
solution = OMMXPySCIPOptAdapter.solve(instance)

# 最適解の分析をする
df_vars = solution.decision_variables_df()
df = pd.DataFrame.from_dict(
    {
        "アイテムの番号": df_vars.index,
        "ナップサックに入れるか？": df_vars["value"].apply(lambda x: "入れる" if x == 1.0 else "入れない"),
    }
)
```

```{list-table}
:header-rows: 1

* - 変数名
  - 説明
* - `instance`
  - 0-1ナップサック問題に対応する `ommx.v1.Instance` オブジェクト
* - `solution`
  - 0-1ナップサック問題をSCIPで解いた計算結果が格納されている `ommx.v1.Solution` オブジェクト
* - `data`
  - 0-1ナップサック問題の入力データ
* - `df`
  - 0-1ナップサック問題の最適解表す `pandas.DataFrame` オブジェクト
```

+++

## ファイルとしてOMMX Artfactを作成する

OMMX Artifactはファイルで管理する方法と、コンテナのように名前で管理する方法がありますが、ここではまずファイルを使った方法を紹介します。OMMX SDKを使って、上記のデータをOMMX Artifact形式の新しいファイル `my_instance.ommx` に保存しましょう。まず `ArtifactBuilder` を用意します。

```{code-cell} ipython3
:tags: [remove-output]

import os
from ommx.artifact import ArtifactBuilder

# OMMX Artifactファイルの名前を指定する
filename = "my_instance.ommx"

# 既にファイルが存在している場合は削除する
if os.path.exists(filename):
    os.remove(filename)

# 1. ビルダーを作成 (v3 では全アーティファクトが SQLite Local Registry に
#    入るので、ビルダーは image_name を要求します)。名前を考えるのが
#    面倒なら `new_anonymous()` で自動生成できます。
builder = ArtifactBuilder.new_anonymous()
```

[`ArtifactBuilder`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder) は主に 2 つのコンストラクタを持ちます。v3 では build したアーティファクトは必ず SQLite Local Registry のエントリとして残り、`.ommx` ファイルとして共有したい場合は build 後に `Artifact.save(path)` を呼びます。

| コンストラクタ | 説明 |
| --- | --- |
| [`ArtifactBuilder.new`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new) | 呼び出し側で image_name を指定する |
| [`ArtifactBuilder.new_anonymous`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.new_anonymous) | `<レジストリ ID8>.ommx.local/anonymous:<ローカルタイムスタンプ>` 形式で名前を自動生成 |
| [`ArtifactBuilder.for_github`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.ArtifactBuilder.for_github) | GitHub Container Registry に合わせてコンテナの名前を決める |

`new_anonymous` のホスト名は `<レジストリ ID8>.ommx.local` 形式で、`.local` (mDNS) link-local TLD を使っているので誤って push しても実際のリモートレジストリには到達しません。先頭のレジストリ ID は各 `LocalRegistry` の初回作成時に一度だけ生成されてメタデータに保存される UUID で、同じレジストリで作られた anonymous artifact は同じ prefix を共有します。アーカイブを共有した際にも「どのレジストリで作られたか」が判別できます。蓄積した anonymous エントリは `ommx artifact prune-anonymous` で一括削除できます (異なるレジストリ ID の prefix も含めて削除されます)。

**タイムスタンプの注意**: 自動生成タグはビルダーの**ローカルタイム** (TZ マーカー無し) です。異なるタイムゾーンの相手にアーカイブを共有すると、受信者は同じ数字を自分のローカルタイムとして読むため、絶対時刻としての意味はマシン間で失われます。タイムゾーンに関係なく安定したタグが必要なら `ArtifactBuilder.new(...)` で明示的に名前を指定してください。

どの方法で初期化しても同じように `ommx.v1.Instance` や他のデータを保存することが出来ます。上で用意したデータを追加してみましょう。

```{code-cell} ipython3
# ommx.v1.Instance オブジェクトを追加する
desc_instance = builder.add_instance(instance)

# ommx.v1.Solution オブジェクトを追加する
desc_solution = builder.add_solution(solution)

# pandas.DataFrame オブジェクトを追加する
desc_df = builder.add_dataframe(df, title="ナップサック問題の最適解")

# JSONに変換可能なオブジェクトを追加する
desc_json = builder.add_json(data, title="ナップサック問題のデータ")
```

OMMX Artifactではレイヤーという単位でデータを管理しますが、各レイヤーは中身がどんな種類のデータなのかを表現するためにMedia Typeを保持しており、`add_instance` などの関数はこれらを適切に設定した上でレイヤーを追加します。この関数は生成したレイヤーの情報を保持した `Description` オブジェクトを返します。

```{code-cell} ipython3
desc_json.to_dict()
```

`add_json` に追加した `title="..."` という部分はレイヤーのアノテーション（注釈）として保存されます。OMMX Artifactというのは人間のためのデータ形式なので、これは基本的には人間が読むための情報です。`ArtifactBuilder.add_*` 関数はいずれも任意のキーワード引数を受け取り、自動的に `org.ommx.user.` 以下の名前空間に変換します。

さて最後に `build` で SQLite Local Registry に publish し、`save` で `.ommx` ファイルにエクスポートします。

```{code-cell} ipython3
# 3. SQLite Local Registry に publish
artifact = builder.build()

# 4. 共有用に .ommx アーカイブにエクスポート
artifact.save(filename)
```

ファイルが出来上がったか確認してみましょう：

```{code-cell} ipython3
import os
print(os.path.exists(filename))
```

あとはこの `my_instance.ommx` を通常のファイル共有の方法で共有すれば、他の人とデータを共有することができます。

+++

## OMMX Artfact形式のファイルを読み取る

次に保存したOMMX Artifactを読み込みましょう。アーカイブ形式で保存したOMMX Artifactを読み込むには [`Artifact.load_archive`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact.load_archive) を使います

```{code-cell} ipython3
from ommx.artifact import Artifact

# ローカルにあるOMMX Artifactファイルを読み込む
artifact = Artifact.load_archive(filename)
```

OMMX Artifactはレイヤーという単位でデータを管理しますが、このレイヤーのデータはマニフェスト（目録）として内包されており、アーカイブファイル全体を読み込まずに確認することが可能です。[`Artifact.layers`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact.layers) によって含まれるレイヤーの `Descriptor` を取得できます。これにはそのレイヤーのMediaTypeとアノテーションが含まれています。

```{code-cell} ipython3
import pandas as pd

# 見やすいように pandas.DataFrame に変換する
pd.DataFrame({
    "Media Type": desc.media_type,
    "Size (Bytes)": desc.size
  } | desc.annotations
  for desc in artifact.layers
)
```

例えばレイヤー3に入っているJSONを取得するには [`Artifact.get_json`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/artifact/index.html#ommx.artifact.Artifact.get_json) を使います。この関数はMedia Typeが `application/json` である事を確認し、バイト列をJSON文字列としてPythonオブジェクトに復元します。

```{code-cell} ipython3
artifact.get_json(artifact.layers[3])
```

```{code-cell} ipython3
:tags: [remove-cell]

# Remove the created OMMX Artifact file to clean up
os.remove(filename)
```
