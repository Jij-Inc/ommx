---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# 変換が何を保存するか

問題の表現を変える目的は、ソルバーに渡す、変数を減らす、特定の条件下で調べる、といった利用者の作業によって異なります。**変換に成功したこと、候補を元の変数で読めること、元の問題の最適解が得られることは別です。** 使う操作の保証が、自分の目的に十分かを確認します。

このページは、InstanceとStateの意味に加え、`evaluate()`で候補の実行可能性と目的値を確認できること、それだけでは最適性を判定できないことを理解している読者向けです。評価の基本は[StateからSolutionへの評価](./solution.md)で説明します。ここでは操作の選び方と結果の解釈を扱い、個々の変換の数式・適用条件はAPI Referenceへ委ねます。実行手順を先に試したい場合は[Instanceを変換し、結果を確認する](../how_to/transform_instance_without_solver.md)を使えます。

## まず、何を変える操作かを確かめる

同じ「変換」でも、何を変え、どの候補を対応付けるかは異なります。

| 操作の例 | 変わるもの | 利用者が確認する点 |
|---|---|---|
| OneHotを通常の等式にする | 同じ条件の表し方 | 変数の定義域とloweringの適用条件 |
| 整数をBinary変数で符号化する | 値を選ぶ変数と、元の変数への対応 | 許す整数範囲を表せるか、補助変数に必要な条件があるか |
| 制約から変数のboundを絞る | 変数の取りうる範囲 | 実行可能な値を除かない更新か、一回の実行でどこまで調べるか |
| 変数を固定する | 探索する候補の範囲 | 元の問題全体か、固定条件付きの部分問題か |
| 制約を緩める | 許す候補の範囲 | 緩和した問題で得た候補が元の条件も満たすか |
| 制約をpenaltyに移す | 比較する目的関数と、現在課す制約 | どの候補にどれだけ罰則が付くか、重みが目的に十分か |

一つの操作で成り立つ説明を、すべての操作へ広げないでください。例えば、元の変数値へ戻せる符号化でも、その値が元の制約を満たすかは別に確認します。

## 制約から分かる変数の範囲を反映する

例えば、$0\leq x\leq10$に加えて通常制約$x\leq3$があるなら、上限を3に絞っても実行可能な候補を除きません。これは任意の値に固定して部分問題を作る操作とは異なり、問題の条件から導ける範囲を変数に反映する操作です。

{meth}`~ommx.Instance.tighten_bounds_simultaneously_once`は、対象となる通常制約を呼び出し開始時のboundで調べ、導いた更新をまとめて一度適用します。更新後のboundで収束まで繰り返すpresolverではなく、完全な実行不可能性検出も約束しません。浮動小数点計算では数値境界付近の候補の判定が変わる場合もあります。対象・許容誤差などのAPI契約に従って使い、更新がなかったことだけで問題が実行可能だと判断しないでください。

[制約から変数のboundを絞る](../how_to/tighten_bounds.md)では、実行前後のboundと変更された変数を確認できます。厳密な適用条件・数式・数値条件・失敗時の扱いは{meth}`~ommx.Instance.tighten_bounds_simultaneously_once_using_constraints`のAPI Referenceで説明します。

## 保証を読む七つの問い

API Referenceの個別の契約を読むときは、次を分けて確認します。

| 問い | 確認する内容 |
|---|---|
| 候補はどう対応するか | 同じ変数か、補助変数を加えるか、固定値・依存式で戻すか |
| 実行可能性はどちら向きに移るか | 変換後の実行可能候補を元へ戻せるか、元の全候補を変換後にも表せるか |
| 目的値はどう対応するか | 値・順序が同じか、符号・定数が変わるか、別のpenalty目的関数か |
| 最適性はどの範囲で移るか | 上の対応が最適解にも十分か、固定した部分問題の中だけの最適性か |
| 数値条件は何か | 定義域、bound、許容誤差、正規化、個別操作の前提 |
| 候補をどのInstanceで評価するか | 対応する元の問題、またはその評価情報を保持した変換済みInstanceか |
| 自分の目的に十分か | 最適値だけが必要か、元のすべての実行可能候補を扱う必要があるか |

例えば最適値が変わらない操作でも、すべての実行可能候補を残すとは限りません。候補の列挙やサンプリング分布を扱う目的では、最適値だけの保証では足りないことがあります。

## 固定した部分問題と、元の問題全体を分ける

Binary変数$x$を最小化する問題の最適値は0です。$x=1$と決めた部分問題では最適値が1になります。

```{code-cell} ipython3
from ommx import Instance

source = Instance.minimize()
x = source.new_binary("x")
source.objective = x

original_candidate = source.evaluate({x.id: 0})
conditioned = source.partial_evaluate({x.id: 1})
conditioned_candidate = conditioned.evaluate({})

assert original_candidate.feasible and original_candidate.objective == 0.0
assert conditioned_candidate.feasible and conditioned_candidate.objective == 1.0
assert conditioned_candidate.state.entries[x.id] == 1.0
```

固定値を復元でき、戻した候補が実行可能でも、元の問題全体で最適とは限りません。上の最適値は、Binaryの二つの値を比較すれば分かるものであり、`evaluate()`が最適性を判定したのではありません。

逆に、元の制約から必ず$x=1$だと分かっている場合の固定は、候補の範囲を余分に狭めない簡約になり得ます。固定操作を一律に非等価とするのではなく、**なぜその値に固定でき、どの集合の中で保証が成り立つか**を考えます。一般のモデルでは、{meth}`~ommx.Instance.partial_evaluate`が成功しただけで残りの制約の実行可能性は分かりません。得た候補を明示的に評価します。

## 不等式の本体を二乗すると、実行可能候補の順序も変わり得る

penaltyの例として、次の問題を考えます。

$$
\min x \quad\text{subject to}\quad x-1\leq0,\quad x\in\{0,1\}.
$$

0と1はどちらも実行可能で、元の目的関数は0を選びます。しかし、不等式の本体をそのまま二乗して重み2を付けると、目的関数は$x+2(x-1)^2$です。

| $x$ | 元の目的値 $x$ | 二乗した本体 $(x-1)^2$ | penaltyを加えた値 |
|---|---|---|---|
| 0 | 0 | 1 | 2 |
| 1 | 1 | 0 | 1 |

実行可能な0にも罰則が付き、順序が逆転しました。「重みを大きくすれば元の最適解が得られる」とは一般にはいえません。これは、違反量だけを二乗する$\max(0,x-1)^2$とは別の操作です。後者ならこの二候補の罰則はともに0です。

OMMXの固定weightのpenalty操作も、通常制約の本体を二乗します。不等式を先に適切なslack変数で等式にするなど、何をpenaltyにするかを判断する必要があります。個別の条件と処理は {meth}`~ommx.FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight`、{meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack` のAPI Referenceを参照してください。重みやslackの条件は、Adapter名だけから自動的に決まるものではありません。

## 複数の操作を使う場合

後の操作へ渡す条件と、最終候補を読むための対応をつなげて確認します。例えば「整数を符号化する→制約をpenaltyに移す→最小化向けに目的関数を変える」では、元の整数値への対応、penalty後の実行可能性、符号を変える前の目的値はそれぞれ別の説明です。

[InstanceClass](./instance_class.md)へのmembershipは要求された表現条件への適合です。[PreparationPolicy](./preparation_policy.md)は使用を許す操作の選択です。Preparation後に`target.contains(instance)`が真でも、元の問題との数理的対応や、実行が返す結果の品質・最適性はその真偽だけからは分かりません。

理想的な数式上の対応と、浮動小数点の許容誤差を含むAPIの契約も分けます。例えばBinary変数の近い値を丸める規則と、通常制約の成立を判定する誤差は、それぞれの公開契約に従います。手元の数候補で変換前後の値が一致したことは、その例の確認であり、任意の問題で同じ保証が成り立つ根拠にはなりません。

## 結果をどの問題に対して読むか

別システムが返した値だけでは、元の制約への実行可能性や目的値は決まりません。対応するInstanceの定義域・制約・目的関数を使って評価します。変換済みInstanceが固定値・依存式・removed制約・出力目的関数を保持していれば、その情報で戻した値や結果を確認できます。ただし、保持した式が後のあらゆる変換に対して不変なコピーであるとは限りません。

元のInstance自体を後で評価・再利用する必要がある場合は、そのInstanceを保持します。変換前後の評価結果を比較するだけなら、事前の評価結果を保存すれば足ります。`prepare()`の例では、後者のためだけにInstanceをcopyしません。

値の出所は[変数の役割と値の復元・補完](./variable_roles.md)、保持した制約・目的値と最適性の表示範囲は[変換後の問題とRemoved constraints](./removed_constraints.md)を参照してください。activeな問題に対する最適性を出力目的関数へ移せることも、removedな制約を含む元の問題全体への最適性保証とは同じではありません。
