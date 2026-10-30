# レジストリでArtifactを共有する

**共有先の名前でArtifactを作り、認証後に`push()`します。** ファイルで渡す場合と違い、配布先へのアクセス権とネットワーク接続が必要です。

Artifactは、複数のデータとその説明をまとめる入れ物です。ここではGitHub Container Registry（`ghcr.io`）を例に、JSONの小さなメモを共有先へ保存し、受信側で取得します。問題や評価結果も同じArtifactに追加できます。ローカルのファイルだけで十分なら、{class}`~ommx.artifact.Artifact`のファイル受け渡し例を使ってください。Experimentはこの操作の前提ではありません。

## 必要な環境と権限

- [インストール手順](../getting_started/install.md)で用意したPython SDK
- 自分がアップロードを許可されたGitHubのアカウントまたはorganizationとリポジトリ
- ログインに使うDocker CLIと、`ghcr.io`への接続

共有するデータの公開範囲と権限を、アップロード前に確認します。GitHubの認証方式、トークンの権限、organizationのSSO条件は[GitHub Container Registryの公式手順](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-to-the-container-registry)に従ってください。トークンをNotebookや共有するソースコードへ書き込まないでください。

認証に使うユーザー名を指定して、ターミナルでログインします。

```bash
docker login ghcr.io -u YOUR-GITHUB-USER
```

OMMXはDockerの認証設定を利用します。環境に`OMMX_BASIC_AUTH_*`による明示的な認証設定がある場合は、そちらが優先されます。レジストリが公開データの匿名取得を許している場合、受信側のログインは不要です。

## 共有先の名前で作成して送信する

`ghcr.io/my-team/my-project/candidate-notes:trial-1`は配布物を指す参照で、末尾の`trial-1`がタグです。以下の`my-team`、`my-project`、タグ`trial-1`を、自分が書き込める対象に置き換えます。例の名前をそのまま実行してもOMMXプロジェクトの配布先には保存できません。既存の配布物を上書きしない、新しいタグを使ってください。

```python
from ommx.artifact import ArtifactDraft

draft = ArtifactDraft.for_github(
    org="my-team",
    repo="my-project",
    name="candidate-notes",
    tag="trial-1",
)
_ = draft.add_json({"trial": 1, "note": "shared record"}, title="summary")
artifact = draft.commit()
print(artifact.image_name)
artifact.push()
```

`commit()`まではLocal Registry（SDKのローカル保存場所）への保存で、`push()`で初めてリモートに送信します。問題や評価結果があれば、`commit()`の前に`draft.add_instance(instance)`や`draft.add_solution(solution)`を追加します。

{meth}`~ommx.artifact.ArtifactDraft.for_github`はGitHubのソースリポジトリ情報も設定しますが、リポジトリや権限そのものを作成するAPIではありません。また、ローカル用の`new_anonymous()`で生成した名前はリモートの共有先にはなりません。

## 受信側で取得して読む

別のマシン、またはその参照がまだないLocal Registryを使うプロセスで実行します。非公開のデータなら、受信側にも読み取り権限とログインが必要です。保存時に付けたタイトル`summary`を手がかりに、JSONメモを選びます。

```python
from ommx.artifact import Artifact

artifact = Artifact.load(
    "ghcr.io/my-team/my-project/candidate-notes:trial-1"
)
layer = next(
    layer
    for layer in artifact.layers
    if layer.annotations.get("org.ommx.user.title") == "summary"
)
summary = artifact.get_json(layer)
assert summary == {"trial": 1, "note": "shared record"}
```

`load()`はLocal Registryにある参照を優先し、なければリモートから取得して保存します。同じタグの更新を毎回取得する処理ではありません。特定の内容を共有・記録する場合は、マニフェスト（収録データの目録）の内容から計算する識別子digestを使い、配布側が確認した`ghcr.io/.../candidate-notes@sha256:...`形式の参照を渡します。使い分けの詳細は{class}`Artifactの名前と内容の識別 <ommx.artifact.Artifact>`を参照してください。

## 取得できない場合

まず、共有された完全な参照、読み書きそれぞれの権限、ログイン先のホスト、ネットワーク接続を確認します。受信側で読むには、ローカルで`commit()`するだけでなく、送信側で`push()`が成功している必要があります。

認証失敗、アクセス拒否、存在しない参照、通信失敗の区別と例外の正確な条件は{meth}`~ommx.artifact.Artifact.load`を参照してください。Local Registryの管理操作は[CLIの案内](../how_to/manage_local_registry.md)へ進んでください。
