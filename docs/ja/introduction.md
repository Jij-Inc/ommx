# OMMXとは？

OMMX（Open Mathematical prograMming eXchange; オミキス）とは、数理最適化を実務に応用する過程で必要となる、ソフトウェア間や人間同士のデータ交換を実現するためのオープンなデータ形式と、それを操作するためのSDKの総称です。

## 数理最適化におけるデータの交換

数理最適化の技術を実務に応用する過程では、多くのデータが生成され、それらのデータを適切に管理・共有することが求められます。数理最適化そのものの研究プロセスとは異なり、実務への応用のプロセスは次の図のように複数のフェーズからなり、それぞれのフェーズにおいてそれぞれの作業に応じたソフトウェアを使う必要があります。

```{figure} ./assets/introduction_01.png
:alt: 数理最適化のワークフロー概略図

数理最適化のワークフロー。
```

例えば、データ分析では標準的なデータサイエンスのツールである `pandas` や `matplotlib`が、定式化ではモデラーと呼ばれる `jijmodeling` や `JuMP` のような数理最適化専用のツールが、最適化の実行には `Gurobi` や `SCIP` などのソルバーが、それぞれ使われます。これらのソフトウェアは、それぞれにとって都合の良いデータ形式を扱うため、相互に運用するにはデータの変換が必要となります。そのため、このような変換は使うツールが増えるほどに組み合わせ的に複雑化していきます。もし標準となるデータ形式が一つ存在すれば、それぞれのツールごとに標準データ形式との相互変換を用意するだけで、どのツールとも連携できるようになり、全体として大幅な効率化が期待できます。

加えて、これらの作業は複数人で分担して行われることが一般的であるため、担当者間でデータを受け渡せる必要があります。人間同士がデータをやり取りする際に重要になるのは、そのデータが何を表し、何のために作られたのかを記述するメタデータです。例えば、ある最適化問題のインスタンスを解いた結果がファイルとして保存されている時、そのファイルに「どの問題を」「どのソルバーで」「どのような設定で」解いたかが記載されていなければ、そのファイルに保存されている情報を別の用途に使うことは出来ません。これを解決するためにはメタデータを付与することが必要ですが、そのメタデータの形式が統一されていなければ、データのやり取りが困難になります。

## OMMXを構成する要素

これらのデータの交換における問題を解決するため、OMMXが開発されました。OMMXは次の4つのコンポーネントで構成されています

- OMMX Message
    
    ソフトウェア間でデータを交換するための、プログラミング言語やOSによらないデータ形式
    
- OMMX Artifact
    
    人間同士でデータを交換するための、メタデータ付きパッケージ形式
    
- OMMX SDK
    
    OMMX MessageとOMMX Artifactを効率的に操作・生成するためのフレームワーク
    
- OMMX Adapters
    
    ソルバーなどの数理最適化ソフトウェアとOMMXのデータ形式を相互に連携するためのソフトウェア群
    

### OMMX Message

OMMX Messageはソフトウェア間でデータ交換を行うために設計されたデータ形式です。これは[Protocol Buffers](https://protobuf.dev/)を用いて定義されており、これにより特定のプログラミング言語やOSに依存しないデータ形式を実現しています。OMMX Messageは、数理最適化の問題 ([`ommx.v1.Instance`](./user_guide/instance.ipynb)) や解 ([`ommx.v1.Solution`](./user_guide/solution.ipynb)) のデータなどを表現するためのスキーマを定義します。
さらに、Protocol Buffersの機能により、ほとんどの実用的なプログラミング言語に対してOMMX Messageを利用するためのライブラリを自動生成することができ、特にPythonとRust向けのライブラリはOMMX SDKの一部として提供されています。

`ommx.v1.Instance` などのデータ構造はMessageと呼ばれ、それぞれのMessageは複数のフィールドを持ちます。例えば、 `ommx.v1.Instance` は次のようなフィールドを持ちます（簡単のために、一部省略しています）:

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

`Instance`をはじめ、決定変数を表すMessage `ommx.v1.DecisionVariable` や目的関数や制約条件として使うための数学的な関数を表すための `ommx.v1.Function` などのMessageが `ommx.v1` という名前空間の元で定義されています。OMMXで定義されているMessageの一覧は、 [OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html) にまとめられています。

一部のソルバーは直接 `ommx.v1.Instance` で定義されたデータを読み込むことができますが、そうでないソルバーに対しては、OMMX Adapterを使ってOMMX Messageをソルバーが扱える形式に変換します。必要になったソフトウェアに対してOMMX Adapterを作成することで、OMMXと連携可能な他のソフトウェアと容易にデータ連携を実現できます。

```{figure} ./assets/introduction_02.png
:alt: OMMX MessageとOMMX Adapterの関係を表す図
:width: 70%

OMMXが実現するソフトウェア間のデータ交換。
```

### OMMX Artifact

OMMX Artifactは、人間同士のデータ交換のために設計された、メタデータ付きパッケージ形式です。これは、コンテナ（Dockerなどのこと）の標準化団体である [OCI (Open Container Initiative)](https://opencontainers.org/) によって定義された OCI Artifactをベースにしています。OCIの標準化では、コンテナは通常のTarアーカイブによって実現され、コンテナの中身であるファイルと共に実行するコマンド等のメタデータが保存されています。これを汎用のパッケージ形式として利用するための仕様が OCI Artifact です。

OCI Artifactでは、パッケージの中身をレイヤーという単位で管理します。一つのコンテナには複数のレイヤーとManifest（目録）と呼ばれるメタデータが含まれます。コンテナを読み込むときは、まずManifestを確認し、その情報に基づいて各レイヤーを読み込むことで、必要なデータを取り出します。各レイヤーはバイナリデータ（BLOB）として保存され、 [Media Type](https://www.iana.org/assignments/media-types/media-types.xhtml) がメタデータとして付与されています。例えば、PDFファイルを保存する場合、 `application/pdf` というMedia Typeが付与されているため、OCI Artifactを読み込むソフトウェアはこのMedia Typeを見て、それがPDFファイルであると認識します。

OMMXでは、OMMX Messageのそれぞれに対して `application/org.ommx.v1.instance` などのMedia Typeを定義し、OMMX MessageをProtocol Buffersでシリアライズしたバイナリを含むOCI ArtifactをOMMX Artifactと呼称しています。厳密に言えば、OMMXはOCI Artifactを何も拡張していないので、OMMX ArtifactをOCI Artifactの一種として扱うことができます。

OCI Artifactをパッケージ形式として利用する利点は、これが全く正規のコンテナとして扱えることです。つまり、[DockerHub](https://hub.docker.com/) や [GitHub Container Registry](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry) をそのまま利用してデータの管理・配布を行うことができます。これにより、例えば、多くのコンテナと同様に、数GBに及ぶベンチマークセットを不特定多数に対して配布することが容易になります。OMMXではこの機能を利用して、代表的なデータセットである [MIPLIB 2017](https://miplib.zib.de/) のデータを[GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017)で配布しています。詳しくは [MIPLIBインスタンスをダウンロードする](./tutorial/download_miplib_instance.ipynb) を参照してください。

```{figure} ./assets/introduction_03.png
:alt: OMMX MessageとOMMX Artifactの関係を表す図

OMMXが実現する人間同士のデータ交換。
```
