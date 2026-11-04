# アーカイブを作ってモデルを読み戻す

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

## 別のSDKで作ったファイルを読む

Local Registryに書き込まずにアーカイブの内容を調べるには {meth}`~ommx.artifact.Artifact.inspect_archive` を使います。マニフェストから分かるのはレイヤーの記述までで、モデルを取り出すときは {meth}`~ommx.artifact.Artifact.import_archive` が必要です。古いアーカイブのレイヤー記述に注釈がある場合、protobuf側にも同じOMMXキーがあればそちらが優先されます。保存済みの試行を移行するときはタイトルや利用者定義の注釈も照合してください。
