# OMMXとは？

OMMX（Open Mathematical prograMming eXchange; オミキス）とは、実務で数理最適化を応用する過程で必要となる、ソフトウェア間や人間同士のデータ交換を実現するためのオープンなデータ形式と、それを操作するためのSDKの総称です。

## 数理最適化におけるデータの交換

数理最適化の技術を実務に応用する過程では、多くのデータが生成され、それらのデータを適切に管理・共有することが求められます。数理最適化そのものの研究プロセスとは異なり、実務への応用のプロセスは次の図のように複数のフェーズからなり、それぞれのフェーズにおいてそれぞれの作業に応じたソフトウェアを使う必要があります。

```{figure} ./assets/introduction_01.png
:alt: 数理最適化のワークフロー概略図

数理最適化のワークフロー。
```

例えばデータの分析では標準的なデータサイエンスのツールである `pandas` や `matplotlib`、定式化ではモデラーと呼ばれる `jijmodeling` や `JuMP` のような数理最適化専用のツール、最適化本体では `Gurobi` や `SCIP` などのソルバーが使われます。これらのソフトウェアはそれぞれにとって都合のいいデータ形式を扱うため、相互に運用するにはデータの変換が必要となります。このような変換は使うツールが増えるほどに組み合わせ的に複雑化していきます。もし標準となるデータ形式が一つ存在すれば、それぞれのツールごとに標準データ形式との相互変換さえ用意すればどのツールとも連携できるようになり、全体として大幅な効率化が期待できます。

加えてこれらの作業は複数人で分担して行われることが一般的で、担当者間でデータが受け渡せる必要があります。人間同士のデータのやり取りにおいて重要になるのが、そのデータが何を表しているのか・何のために作ったデータなのかを記述するためのメタデータです。例えば、ある最適化問題のインスタンスを解いた結果がファイルとして保存されている時、そのファイルがどの問題をどのソルバーでどのような設定で解いたものが記述されていなければ、そのファイルを別の用途に使うことは出来ません。これを解決するためにメタデータを付与することが必要ですが、そのメタデータの形式が統一されていないと、データのやり取りが困難になります。

## OMMXを構成する要素

これらのデータの交換における問題を解決するため、OMMXが開発されました。OMMXは次の4つのコンポーネントで構成されています

- [OMMX Message](./ommx_message/architecture.md)
    
    ソフトウェア間でデータを交換するための、プログラミング言語やOSによらないデータ形式
    
- [OMMX Artifact](./ommx_artifact/architecture.md)
    
    人間同士でデータを交換するための、メタデータ付きパッケージ形式
    
- OMMX SDK
    
    OMMX MessageとOMMX Artifactを効率的に操作・生成するためのフレームワーク
    
- OMMX Adapters
    
    ソルバーなどの数理最適化ソフトウェアとOMMXのデータ形式を相互に連携するためのソフトウェア群
    

### OMMX Message

OMMX Messageはソフトウェア間でデータを交換することを目的として設計されたデータ形式です。これは[Protocol Buffers](https://protobuf.dev/)を使って定義することでプログラミング言語やOSに依存しないデータ形式を実現しています。OMMX Messageは、数理最適化の問題 ([`ommx.v1.Instance`](./ommx_message/instance.ipynb)) や解 ([`ommx.v1.Solution`](./ommx_message/solution.ipynb)) のデータを表現するためのスキーマを定義します。
Protocol Buffersの機能によってほとんどの実用的なプログラミング言語に対してOMMX Messageを利用するためのライブラリを自動生成することができ、特にPythonとRustに対してはOMMX SDKの一部として提供されています。

`ommx.v1.Instance` などのデータ構造はMessageと呼ばれ、それぞれのMessageは複数のフィールドを持ちます。例えば `ommx.v1.Instance` は次のようなフィールドを持ちます（簡単のためにいくつかを省略しています）:

```protobuf
message Instance {
  // 決定変数
  repeated DecisionVariable decision_variables = 2;
  // 目的関数
  Function objective = 3;
  // 制約条件
  repeated Constraint constraints = 4;
  // 最大化・最小化
  Sense sense = 5;
}
```

決定変数を表すMessage `ommx.v1.DecisionVariable` や目的関数や制約条件として使うための数学的な関数を表すための `ommx.v1.Function` などのMessageが `ommx.v1` という名前空間の元で定義されています。OMMXで定義されているMessageのリストが [OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html) にまとまっています。

一部のソルバーは直接 `ommx.v1.Instance` で定義されたデータを読み込むことができますが、そうでないソルバーに対してはOMMX Adapterを使ってOMMX Messageをソルバーが扱える形式に変換します。必要になったソフトウェアに対してOMMX Adapterを作成することでOMMXと連携出来る他のソフトウェアと容易に連携できます。

```{figure} ./assets/introduction_02.png
:alt: OMMX MessageとOMMX Adapterの関係を表す図
:width: 70%

OMMXが実現するソフトウェア間のデータ交換。
```

より詳しい設計については [OMMX Messageの設計](./ommx_message/architecture.md) を参照してください。

### OMMX Artifact

OMMX Artifactは人間同士のデータ交換のために設計されたメタデータ付きのパッケージ形式です。これはコンテナ（Dockerなどのこと）の標準化団体である [OCI (Open Container Initiative)](https://opencontainers.org/) によって定義された OCI Artifactをベースにしています。OCIの標準化ではコンテナというのは通常のTarアーカイブによって実現され、コンテナの中身であるファイルと共に実行するコマンド等のメタデータが保存されています。これを汎用のパッケージ形式として利用するための仕様が OCI Artifact です。

OCI Artifactではレイヤーという単位でパッケージの中身を管理します。一つのコンテナには複数のレイヤーとManifest（目録）と呼ばれるメタデータが含まれます。コンテナを読み込むときはまずManifestを確認し、その情報を元にレイヤーを読み込むことで必要なデータを取り出します。各レイヤーはバイナリデータ（BLOB）として保存されていますが [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml) がメタデータとして付与されています。例えばPDFファイルを保存するときは `application/pdf` というMedia Typeが付与されているので、OCI Artifactを読み出すソフトウェアはそのMedia Typeを見てPDFファイルであることを認識します。

OMMXではOMMX Messageのそれぞれに対して `application/org.ommx.v1.instance` などのMedia Typeを定義し、OMMX MessageをProtocol Buffersとしてシリアライズしたバイナリを含んだOCI ArtifactをOMMX Artifactと呼称しています。正確に言えばOMMXはOCI Artifactを何も拡張していないので、OMMX ArtifactはOCI Artifactの一種として扱うことができます。

パッケージ形式としてOCI Artifactを利用する利点は、これが全く正規のコンテナとして扱えることです。つまり[DockerHub](https://hub.docker.com/) や [GitHub Container Registry](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry) をそのまま利用してデータの管理・配布が行えます。多くのコンテナと同様に、数GBになるようなベンチマークセットを不特定多数に対して配布することが容易です。OMMXではこの機能を利用して、代表的なデータセットである [MIPLIB 2017](https://miplib.zib.de/) のデータを[GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017)で配布しています。詳しくは [MIPLIBインスタンスをダウンロードする](./tutorial/download_miplib_instance.md) を参照してください。

```{figure} ./assets/introduction_03.png
:alt: OMMX MessageとOMMX Artifactの関係を表す図

OMMXが実現する人間同士のデータ交換。
```

より詳しい設計については [OMMX Artifactの設計](./ommx_artifact/architecture.md) を参照してください。
