# OMMX Python SDK 2.8.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.8.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.8.0)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。

## バグ修正

### QPLIBの二次係数と配布Artifactを修正 (2.8.0, [#1209](https://github.com/Jij-Inc/ommx/pull/1209))

`Instance.load_qplib()`と`ommx.qplib.load_file()`で、目的関数と制約の
対角項・交差項にQPLIB形式の係数`1/2`を適用するようになりました。
従来は二次項の寄与が2倍になっていました。線形項・定数項・上下限は元の値を保ちます。

`ommx.dataset.qplib()`は、修正済みパーサーで公式ファイルから再生成した
`ghcr.io/jij-inc/ommx/v2.8/qplib:{tag}`を読み込みます。
Rustのローダーも同じ配布版を使用します。バージョンなしの旧配布先のキャッシュが
修正版より優先されることはありません。手動で保存したインスタンスに修正を
反映するには、元の`.qplib`ファイルから読み込み直してください。

公開済みの参照先は固定し、patch更新では採用済みの配布版を維持します。
MIPLIBは引き続きv2.7の配布版を使用します。取得先は
[QPLIBチュートリアル](../tutorial/download_qplib_instance.ipynb)を参照してください。

この修正により、目的関数値や実行可能領域を含む数学モデルが変わるため、
更新後は影響を受けるQPLIBインスタンスの結果を再計算してください。
2.8.0の各ソルバーアダプターは、v2系のOMMX 2.8.0以降を必要とします。
