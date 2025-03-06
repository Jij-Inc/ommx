# OMMX Adapterを実装する

[複数のAdapterで最適化問題を解いて結果を比較する](../tutorial/switching_adapters)で触れた通り、OMMX Python SDKにはAdapterを実装するための抽象基底クラスが用意されており、これを継承する事で共通の仕様に沿ったAdapterを実装する事ができます。OMMXはAdapterの性質に応じて二つの抽象基底クラスを用意しています。

- [`ommx.adapter.SolverAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SolverAdapter): 一つの解を返す最適化ソルバーのための抽象基底クラス
- [`ommx.adapter.SamplerAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SamplerAdapter): サンプリングベースの最適化ソルバーのための抽象基底クラス

複数の解が得られるソルバーは、特に複数得られたサンプルのうち最善のものを選択することによって、自動的に単一の解を返すソルバーと見なす事ができるため、`SamplerAdapter` は `SolverAdapter` を継承しています。Adapterを作るときにどちらを実装するか悩んだら、出力される解の数を見て、一つの解を返すなら `SolverAdapter`、複数の解を返すなら `SamplerAdapter` を継承すると良いでしょう。たとえば [PySCIPOpt](https://github.com/scipopt/PySCIPOpt) などの数理最適化ソルバーは一つの解を返すため `SolverAdapter` を使い、[OpenJij](https://github.com/OpenJij/OpenJij) などのサンプラーは複数の解を返すため、`SamplerAdapter` を使います。

OMMXでは `ommx.adapter.SolverAdapter` を継承したクラスを **Solver Adapter**、`ommx.adapter.SamplerAdapter` を継承したクラスを **Sampler Adapter** と呼びます。
またここでの説明のため、PySCIPOptやOpenJijのようにAdapterがラップしようとしているソフトウェアのことをバックエンドソルバーと呼びます。

## Adapterの処理の流れ

Adapterの処理は大雑把にいうと次の3ステップからなります：

1. `ommx.v1.Instance` をバックエンドソルバーが読める形式に変換する
2. バックエンドソルバーを実行して解を取得する
3. バックエンドソルバーの解を `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換して返す

2.はバックエンドソルバーの使い方そのものなので、これは既知としてこのチュートリアルでは扱いません。ここでは 1. と 3. をどのように実装するかを説明します。

多くのバックエンドソルバーが数学的な数理最適化問題を表すための必要な最小限の情報だけを、アルゴリズムに応じた形で受け取るように作られているのに比べて、`ommx.v1.Instance` はデータ分析の一部として数理最適化を行うことを想定しているためより多くの情報を持っています。なのでステップ 1. では多くの情報を削ぎ落とすことになります。またOMMXでは決定変数や制約条件は連番とは限らないIDで管理されていますが、バックエンドソルバーによっては名前で管理されいたり、連番で管理されていることがあります。この対応関係は 3. の処理で必要になるのでAdapterが管理しておく必要があります。

逆にステップ 3. では `ommx.v1.Solution` や `ommx.v1.SampleSet` はバックエンドソルバーの出力だけからは構築できないので、まずバックエンドソルバーの返した解と 1. の時の情報から `ommx.v1.State` あるいは `ommx.v1.Samples` を構築し、それを `ommx.v1.Instance` を使って `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換します。

## Solver Adapterを実装する

ここでは PySCIPOpt を例としてSolver Adapterを実装してみましょう。なお完全な例は [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) を確認してください。

### 決定変数の変換

```python
print("hi")
```

### 目的関数と制約条件の変換

### `OMMXPySCIPOptAdapter` の実装

## Sampler Adapterを実装する

ここでは OpenJij を例としてSampler Adapterを実装してみましょう。なお完全な例は [ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter) を確認してください。
