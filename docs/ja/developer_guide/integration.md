# 別のツール・言語をOMMXへ接続する

このページは、モデラー、業務ツール、Rustプログラム、別言語のreader/writerを
OMMXへ接続する実装者向けです。Python Tutorialの通読は前提ではありません。
必要なのは、最適化問題・候補の意味と、自分が接続する境界の契約です。

共通概念のGuideにはPython例もありますが、Rust・他言語の読者はそれを実行せず、
数式・表・説明から意味を確認して、下記の版を固定したAPI・仕様へ進めます。

## 接続する境界を選ぶ

| 目的 | 引き受ける責任 | 次の参照先 |
|---|---|---|
| モデラーから問題を出す | 変数のdomain、目的関数、制約、名前とIDをOMMXの問題へ対応付ける | Instance・Function・特殊制約 |
| Rustで問題や候補を操作する | SDKの公開型と適用条件を使って構築・評価・変換する | 版を固定したRustdoc |
| 別言語でMessageを読む・書く | Media Typeに対応するwire schemaと問題の意味を満たす | Message定義・概念Guide |
| 問題・結果・分析データを一緒に渡す | レイヤーと型、内容識別子、archiveをArtifact仕様に従わせる | Artifact仕様 |
| 外部Python型をExperimentに添付する | 型を所有するパッケージでCodecと互換性を定める | Codec Protocol |
| 実行backendへ接続する | exact input、変数encode、候補decodeと評価を実装する | Adapter Developer Guide |

すべてを実装する必要はありません。例えばMessageを読むだけのツールが、Adapterや
Experimentを実装する必要はありません。ArtifactやExperimentはcore操作の前提ではなく、
保存・交換や記録を行う場合の選択肢です。

## 最初に合わせる共通の意味

[Instance](../user_guide/instance.md)は問題の変数・式・制約・senseを持ち、
[Solution](../user_guide/solution.md)はその問題で候補を評価した結果です。
保持されている変数がすべて独立の探索変数とは限らない点は
[変数の役割](../user_guide/variable_roles.md)で確認します。

ソフトウェア間で、IDに対応する変数、変数の範囲・種類、制約の成立条件、報告目的値の
意味を一致させます。候補値の復元、実行可能性の評価、最適性の報告は別の責任です。
値を読み戻せたことだけで、元の問題の実行可能性や大域的最適性は保証されません。

変換を使う場合は[InstanceClass](../user_guide/instance_class.md)と
[PreparationPolicy](../user_guide/preparation_policy.md)を入口にします。
class membershipは表現の条件であり、元の問題との対応は
[各変換の保証](../user_guide/transformation_guarantees.md)から確認します。
backend用の係数への整形と、問題そのものの変換も分けて扱ってください。

## 版を合わせて実装する

[Reference](../reference/index.md)にPython/Rust API、Message、Artifact、Codec、Adapterの
固定版の参照先をまとめています。Python `3.0.0b7`とRust `3.0.0-beta.7`は独立した版です。
wire schemaの版もSDKの版ではありません。公開版を対象にする実装で、根拠を示さずに
`main`の追加APIへ依存しないでください。

別言語での読み書きでは、対象schemaのフィールドを読むことに加え、各root objectの
公開契約に従って問題や候補を構築します。最小の往復テストを用意して、問題・候補・型や
添付データの対応が失われないことを確認してください。Artifactの受信テストでは、
送信側のRegistryやメモリに依存しない空の受信環境を使います。
