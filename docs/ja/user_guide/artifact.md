# Artifactの保存と識別

**Artifactは、問題・候補の評価結果・分析データなどをまとめて保存・受け渡すための入れ物です。** 問題の評価や変換、Adapterの利用だけなら、Artifactを作る必要はありません。

初めてファイルを作る場合は[問題と評価結果をファイルで受け渡すTutorial](../tutorial/share_in_ommx_artifact.md)へ進んでください。このページでは、どの状態のデータを、どこに、どの名前で保存しているかを整理します。

MPS入力に対応する外部ツールへ現在の数理モデルを渡す場合は、[MPS出力How-to](../how_to/export_mps.md)を使えます。MPSは変数・制約のラベルや評価用情報のすべてを保持する保存形式ではありません。OMMXとして後から取り出すための保存・受け渡しにはArtifactを使います。

## 入れるデータと、その関係

Artifactは、データ本体と、その種類や説明をまとめた目録を持ちます。例えば、`Instance`、その候補を評価した`Solution`、分析メモのJSONを一つにまとめられます。各データ本体を**レイヤー**、目録を**マニフェスト**と呼びます。

レイヤーの**Media Type**は、データをどう解釈するかを示します。`add_instance()`と`get_instance()`のような型別APIを使うと、保存形式を手作業で決めずに問題を保存・復元できます。レイヤーの説明などは**annotation**として記録できます。

複数の問題や結果を保存しただけで、任意の二つの対応関係が自動的に決まるわけではありません。どの入力からどの結果が得られたかなど、利用側に必要な関係も明示的に残します。{class}`~ommx.artifact.Descriptor`はレイヤーのMedia Type、サイズ、内容の識別子などを持ちます。Tutorialでは、その識別子を分析メモにも保存しています。

## Draftと確定したArtifact

{class}`~ommx.artifact.ArtifactDraft`は、追加するレイヤーや説明を組み立てるためのオブジェクトです。`commit()`すると一覧が確定し、{class}`~ommx.artifact.Artifact`としてLocal Registry（SDKのローカル保存場所）から参照できるようになります。確定した内容に後からレイヤーを付け足す操作ではありません。違う内容を作る場合は、新しいDraftを作ります。

`commit()`はファイルの書き出しやリモートへの送信ではありません。確定したArtifactに対して、必要なら`save()`や`push()`を呼びます。Draftの操作でもデータがローカルに保存されるため、`commit()`前ならディスクに何も残らない、という意味でもありません。

## 三つの保存・受け渡し先

| 場所 | 役割 | 主な操作 |
| --- | --- | --- |
| Local Registry | そのマシンでの永続的な保存・再利用 | `commit()`、`Artifact.load()` |
| `.ommx`ファイル | 通常のファイル転送での受け渡し | `save()`、`Artifact.import_archive()` |
| リモートレジストリ | チームや別のマシンへの配布 | `push()`、ローカルにない参照の`Artifact.load()` |

**Local Registry**はSDKが利用するローカル保存場所です。標準の場所を使うほか、プロセスの起動時に`OMMX_LOCAL_REGISTRY_ROOT`で別の場所を指定できます。既存の保存場所を変更すると、その場所にあるデータだけが参照対象になります。ファイルを移動したことにはなりません。

`Artifact.import_archive()`はファイルの内容をLocal Registryへ取り込みます。後で同じ名前を`Artifact.load()`に渡して取り出せます。内容を取り込まず目録だけ確認したい場合は、{meth}`~ommx.artifact.Artifact.inspect_archive`を使います。これはレイヤーのデータ本体を復元するAPIではありません。

リモートレジストリはOCI形式のデータを配布するサーバーです。認証や公開範囲はそのサービスに従います。Artifactは計算を実行するコンテナではなく、データを入れたパッケージです。Dockerなどの実行環境を使って最適化プログラムを起動するという意味ではありません。具体的な操作は[レジストリでArtifactを共有する](../how_to/share_artifact_registry.md)で説明します。

(artifact-identity)=
## 名前・タグと内容の識別子

`ghcr.io/my-team/my-project/candidates:trial-1`のような参照は、保存先、名前、タグからなります。`trial-1`がタグです。名前やタグは人がデータを探す手がかりですが、同じタグが将来も同じ内容を指すとは限りません。

一方、**digest**は内容から計算する識別子です。マニフェストのdigestはArtifactの内容の一覧を識別し、各レイヤーにもそれぞれdigestがあります。特定の内容を指したい場合は、タグだけでなく対象のdigestも記録します。マニフェストとレイヤーのdigestは同じものではありません。アーカイブの目録からは{attr}`~ommx.artifact.ArchiveManifest.manifest_digest`でマニフェストのdigestを確認できます。

`Artifact.load(name)`は、まずLocal Registryを参照し、見つからなければリモートから取得します。すでにあるタグを指定しても、毎回リモートの最新内容を取り直す操作ではありません。特定の配布物を使い続ける場合は、配布側が示すdigestによる参照を使います。

名前を考えずローカルで始めるには、`ArtifactDraft.new_anonymous()`を使えます。SDKが`.ommx.local`を含む名前を生成します。この名前はリモート配布先ではありません。ファイルとしては受け渡せますが、リモートへ配布する予定なら、作成時から配布先の名前を指定します。自動名は整理・削除の対象になりうるため、残したいデータの管理は{ref}`Local Registryの整理 <experiment-cleanup-workflow>`も確認してください。

## Experimentとの違い

Artifactはデータをまとめる汎用の保存形式です。試行のまとまりや各試行の条件・結果も整理したい場合は、Artifactを保存の土台として使う[Experiment](experiment.md)があります。ExperimentはRunやSolve/Samplingの関係を記録しますが、汎用Artifactへデータを追加するだけでExperimentになるわけではありません。

どちらも、保存しなかった環境やソースコードを補完したり、保存済みの評価結果だけから計算の再実行を保証したりはしません。何を後で読みたいかに応じて、必要な入力や説明を残します。

## SDKと形式のバージョン

Python SDKのバージョンは、呼び出すAPIの版です。使用する版は[インストール手順](../getting_started/install.md)で確認してください。問題や評価結果の保存形式（OMMX Messageのwire schema）、Artifactそのものの形式・種別とは別です。例えば、汎用Artifactの種別`application/org.ommx.v1.artifact`にある`v1`は、SDKのメジャーバージョンではありません。

言語をまたぐ読み書きでは、[Artifact仕様](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md)と[Message定義](https://github.com/Jij-Inc/ommx/tree/main/proto)を基準にします。このBookと同じmainの仕様を参照してください。別の実装とデータを交換する場合は、その実装が対応する形式も確認します。Python APIの正確な条件は{mod}`ommx.artifact`を参照してください。
