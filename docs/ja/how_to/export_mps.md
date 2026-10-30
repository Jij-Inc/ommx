---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# InstanceをMPSファイルで外部ツールへ渡す

外部ソルバーや既存のファイル処理へ問題を渡すには、{meth}`~ommx.Instance.save_mps`でMPSファイルを作れます。**MPSに出力するのは現在課している数理モデルです。** OMMXのラベルや評価用情報も残して保存・受け渡す場合は、[Artifact](../user_guide/artifact.md)を使います。

[インストール手順](../getting_started/install.md)に従ってmainのPython SDKを用意してください。このページでファイルを作るためにAdapterやソルバーは必要ありません。出力先のフォルダーに書き込みできる環境を用意してください。

## 小さな問題を出力する

次の例だけで、二つの商品を同時には選べない問題を作り、カレントディレクトリに`selection.mps`を出力できます。既存の問題を使う場合は、[読み込みHow-to](./load_existing_instance.md)で得たInstanceに対して同じ`save_mps()`を呼びます。

```{code-cell} ipython3
from pathlib import Path
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("selected", subscripts=[0])
y = instance.new_binary("selected", subscripts=[1])
instance.objective = 3 * x + 2 * y
instance.add_constraint(x + y <= 1, "at_most_one")

path = Path("selection.mps")
instance.save_mps(str(path), compress=False)
assert path.is_file()
```

このファイルを、利用する外部ツールのMPS入力機能へ渡します。`compress=False`はプレーンテキストでの出力です。gzip圧縮で渡したい場合は、例えば`instance.save_mps("selection.mps.gz")`とします。圧縮の有無は拡張子ではなく`compress`引数で決まり、既定値は`True`です。同じパスにある既存ファイルは上書きされるため、残したいファイルとは別の出力先を指定してください。

出力の失敗で不完全なファイルが残ることがあり、gzip圧縮では正常に戻っても完了を保証できません。受け渡し前にファイルを読み込めることを確認してください。詳しい失敗時の扱いは{meth}`~ommx.Instance.save_mps`のAPI Referenceを参照してください。

ここまででファイルを渡す準備は完了です。外部ツールの実行方法や結果の取り込みは、そのツール側の手順に従います。

## 出力できるモデルを確認する

出力対象の変数がBinary・Integer・Continuousで、目的関数と通常制約が展開済みの二次以下の多項式であるモデルを出力APIが受け付けます。三次以上の多項式や一般の非線形式は対象外です。また、数学的には二次式でも、保持している式の表現によってはそのまま出力できません。例えば整数冪の扱いは{meth}`~ommx.Function.powi`のAPI Referenceを参照してください。

二次項を含む出力では、受け取るツールがMPSの二次拡張に対応していても、係数の解釈が一致するとは限りません。求解前に、取り込まれた目的関数・制約が元の式と一致するか確認してください。現行の出力APIが使う係数規約は、{meth}`~ommx.Instance.save_mps`のAPI Referenceで説明します。

{class}`~ommx.IndicatorConstraint`・{class}`~ommx.OneHotConstraint`・{class}`~ommx.Sos1Constraint`が現在課されている場合は、出力前に通常制約へ変換（lowering）します。{meth}`~ommx.Instance.lower_special_constraints`はInstance自体を変更するため、元のモデルも残す場合は`instance.copy()`で作ったコピーを変換します。変換にはboundなどの条件があり、loweringだけでMPS出力への適合が保証されるわけではありません。必要な変換は{meth}`~ommx.Instance.prepare`から確認してください。

対応する式・変数の正確な条件と出力時の扱いは、{meth}`~ommx.Instance.save_mps`のAPI Referenceで確認してください。

## OMMXとして保存する場合との使い分け

MPSは現在の目的関数・通常制約と、それらで使う変数の種類・boundを渡すための形式です。OMMXが持つ情報のすべては保存されません。

- 変数・制約の名前、添字、説明などのラベルは保持されません。上の`selected`や`at_most_one`も、MPSではIDに基づく名前になります。
- 現在課す条件から外した制約（removed）、結果報告用に保持する出力目的関数、評価指標の式である`NamedFunction`は出力されません。現在のモデルで使っていない変数も省かれます。

そのため、MPSを再読み込みするだけでは、変換前のラベルや評価用情報を復元できません。外部ツールへMPSを渡す場合も、元のInstanceを残しておくと結果を解釈する際に使えます。

| 目的 | 保存方法 |
|---|---|
| MPS入力に対応する外部ツールへ現在のモデルを渡す | `Instance.save_mps()` |
| OMMXのInstanceや評価結果をまとめて保存し、後から取り出す・受け渡す | [ArtifactのTutorial](../tutorial/share_in_ommx_artifact.md) |

Instanceの問題名はMPSの問題名として使われますが、それ以外にも失われる情報があります。情報損失と変数の選別の全一覧はAPI Referenceに委ね、保存の目的に合わせて形式を選びます。
