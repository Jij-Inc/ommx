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

# 既存の問題をInstanceとして読み込む

公開ベンチマークや手元のファイルを読み込み、問題を表す`Instance`を取得します。**入手と求解は別の操作です。** 読み込んだ問題は、手元の候補の評価や、必要に応じた変換・求解へ使えます。

[インストール手順](../getting_started/install.md)に従ってPython SDKを用意してください。以下の取得・読み込みにAdapterは使いません。

## 公開ベンチマークから取得する

初回の取得には、公開レジストリ`ghcr.io`へのネットワーク接続が必要です。

### QPLIB

QPLIBは二次計画問題のライブラリです。例えば、問題番号`0018`を指定して取得できます。番号は先頭のゼロを含む文字列で渡します。

```{code-cell} ipython3
from ommx import dataset

qplib_instance = dataset.qplib("0018")
print(qplib_instance.title)
print(f"決定変数の数: {len(qplib_instance.decision_variables)}")
```

これで問題を`Instance`として扱えます。候補の値も用意されている場合は、[既存の問題と候補解を評価するTutorial](../tutorial/evaluate_existing_candidate)で目的値と制約の充足を確認できます。

### MIPLIB 2017

混合整数計画問題のMIPLIB 2017も同じ操作で取得します。こちらは問題名を指定します。

```{code-cell} ipython3
from ommx import dataset

miplib_instance = dataset.miplib2017("neos-1122047")
print(miplib_instance.title)
print(f"決定変数の数: {len(miplib_instance.decision_variables)}")
```

データはローカルの保存領域（Local Registry）へ取り込まれ、同じ参照のデータが保存済みなら再利用されます。MIPLIB/QPLIB全体のアーカイブを先にダウンロードする必要はありません。

問題番号・問題名や元データの説明は、[QPLIB](https://qplib.zib.de/)と[MIPLIB 2017](https://miplib.zib.de/)の公式サイトで確認できます。すべての元データがOMMX配布に含まれるとは限りません。公開対象と入力形式の制限は、後述の配布記録を参照してください。

## 手元の問題ファイルを読み込む

すでにファイルがある場合は、その形式に対応したローダーを使います。以下の`problem.mps`または`QPLIB_0018.qplib`を、実際のファイルのパスに置き換えてください。

MPSファイルの場合：

```python
from ommx import Instance

instance = Instance.load_mps("problem.mps")
```

QPLIBファイルの場合：

```python
from ommx import Instance

instance = Instance.load_qplib("QPLIB_0018.qplib")
```

これはファイルをOMMXの問題表現へ読み込む操作であり、候補を探索する操作ではありません。対応する入力とエラーは{meth}`~ommx.Instance.load_mps`、{meth}`~ommx.Instance.load_qplib`を参照してください。問題ファイルだけを読み込んでも、候補の変数値は得られません。

## モデラーやOMMX Artifactから受け取る

モデラーがOMMXの`Instance`を出力する場合は、そのオブジェクトを以降の評価・変換・Adapter呼び出しへ使います。出力手順とSDKの互換性は、例えば[JijModelingの公式ドキュメント](https://jij-inc-jijmodeling-tutorials-ja.readthedocs-hosted.com/ja/latest/introduction.html)など、モデラー側で確認し、[インストール手順](../getting_started/install.md)で用意した環境と合わせてください。

OMMX Artifactは、問題・候補・結果などをまとめて受け渡す形式です。`Instance`を含む`.ommx`ファイルを受け取った場合は、次のように読み込みます。

```python
from ommx.artifact import Artifact

artifact = Artifact.import_archive("problem.ommx")
instance = artifact.instance
```

`import_archive()`は、ファイルの内容をLocal Registryへ永続的に取り込みます。この例の`instance`は最初の問題です。複数の問題を含む場合の選び方は{meth}`~ommx.artifact.Artifact.get_instance`、取り込みの詳細は{meth}`~ommx.artifact.Artifact.import_archive`を参照してください。Artifactの作成・共有の手順は別の課題であり、ここで問題を入手するための前提ではありません。

## 配布とバージョンを確認する

公開ベンチマークのローダーは、次の配布を読み込みます。

| ローダー | 配布先 | 配布に含まれる修正と記録 |
|---|---|---|
| `dataset.miplib2017(name)` | `ghcr.io/jij-inc/ommx/v2.7/miplib2017:{name}` | MPSの整数変数の境界を修正した配布。[v2.7の配布記録](https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md) |
| `dataset.qplib(tag)` | `ghcr.io/jij-inc/ommx/v2.8/qplib:{tag}` | 二次係数を修正した配布。[v2.8の配布記録](https://github.com/Jij-Inc/ommx/blob/b4cffe9f1ce5323c15de67eca55b05d9f87285a0/rust/dataset/distributions/v2.8/README.md) |

ここでの`v2.7`や`v2.8`は**データセット配布の識別子**であり、SDKの版を指定しているのではありません。ローダーは表の配布を使い、バージョンなしの旧配布は使いません。どのデータを使ったかを記録するときは、問題名だけでなくこの配布先も残してください。配布記録には元データ、公開対象、モデルの内容を識別するダイジェストが記載されています。

次のバージョンは別々のものです。

- **SDKのバージョン**：Pythonから呼ぶAPIを定めます。使用する版は[インストール手順](../getting_started/install.md)で確認してください。
- **Messageのwire schema**：問題や解をシリアライズしたデータの形式です。配布済みデータにある`application/org.ommx.v1.instance`の`v1`は、SDKのメジャーバージョンではありません。対応するスキーマは[Message定義](https://github.com/Jij-Inc/ommx/tree/main/proto)で確認できます。
- **Artifactの形式**：Messageなどをまとめて受け渡すパッケージの形式です。Python APIとは独立した[Artifact仕様](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md)が定めています。

## 入手後に進む先

- 名前や説明を整える：{class}`~ommx.Instance`のAPI Referenceにあるメタデータ編集例。
- MPS入力に対応する外部ツールへ渡す：[InstanceをMPSファイルで出力する](./export_mps.md)。現在のモデルを出力する形式であり、OMMXのラベルや評価用情報のすべては保持されません。
- 手元の候補を検査する：[既存の問題と候補解を評価する](../tutorial/evaluate_existing_candidate)。
- 表現を変えて検査する：{meth}`~ommx.Instance.prepare`。
- ソルバーへ渡す：{doc}`Adapterの一覧と入力契約 </api/adapters>`から使うAdapterを選ぶ。

データに付いた作者・ライセンス・出典などは`instance.authors`、`instance.license`、`instance.annotations`で確認できます。配布元の既知の目的値などは問題についての情報であり、手元の候補の実行可能性や最適性を保証するものではありません。
