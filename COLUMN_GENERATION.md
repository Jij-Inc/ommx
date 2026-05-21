# OMMX Column Generation Proposal

## 背景

OMMX で MIPLIB の `.dec` ファイルを扱えるようにすると、MIPLIB が提供する分割情報を OMMX の内部 ID に対応付けられる。これにより、単に分割情報を保存するだけでなく、Dantzig-Wolfe 分解や column generation の実装に利用できる。

一方で、Dantzig-Wolfe 分解を OMMX 本体に直接入れるのは現時点では重い。Restricted Master Problem (RMP) は LP として HiGHS で解くのが自然であり、HiGHS 依存を OMMX core に入れるべきではない。また、pricing problem は問題ごとに構造が大きく異なるため、完全に汎用化するには時間がかかる。

そのため、まずは OMMX 本体とは別の実験的 Python パッケージとして `ommx-column-generation` を作り、working example から始める方針を取る。

## 目的

この提案の目的は、OMMX における分割情報の形式を、実際に column generation で使う側から逆算して設計することである。

最初から高性能な汎用 Dantzig-Wolfe solver を目指すのではなく、以下を満たす最小構成を目指す。

- OMMX の `Instance` と分割情報から RMP を構築できる
- RMP を `ommx-highs-adapter` で解ける
- RMP の双対値を pricing callback に渡せる
- pricing 側はユーザーが自由に実装できる
- pricing 解から column を生成し、RMP に追加できる
- MIPLIB `.dec` 由来の分割情報を使った working example を作れる

## パッケージ構成

`ommx-column-generation` は OMMX 本体ではなく、独立した Python パッケージとして提供する。

想定する依存関係は次の通り。

```text
ommx-column-generation
  depends on ommx
  depends on ommx-highs-adapter
```

OMMX 本体は以下を担当する。

- `Instance`
- `ParametricInstance`
- `Solution`
- `SampleSet`
- 将来的な分割情報の永続化形式

`ommx-column-generation` は以下を担当する。

- column generation の反復制御
- RMP の構築
- RMP の解と双対値の取得
- pricing callback との接続
- column pool の管理
- 実験的な `.dec` loader / decomposition helper

HiGHS 依存や column generation の実験的 API は OMMX 本体に入れない。

## 分割情報

分割情報には複数の入力経路があり得る。

1つは MIPLIB の `.dec` ファイルである。`.dec` は入力フォーマットとして扱い、OMMX 内では OMMX 固有の分割情報に正規化する。

`.dec` は制約名ベースの形式なので、ロード時に MPS の row name と照合し、OMMX の constraint ID に変換する。変換後は MPS 名に依存しない。

もう1つの経路は、JijModeling のような数式レベルのモデラーから分割情報を得る方法である。モデラーは係数行列に展開される前の添字、`forall`、条件式、和の構造を保持しているため、問題によっては巨大な係数情報を構築せずに、数式構造だけから separable constraints と overlapped constraints を検知できる。

この経路には以下の利点がある。

- 巨大な MPS / matrix を作る前に分割構造を検知できる
- 添字や条件式に由来する block 構造を失わずに扱える
- モデル生成時点の意味情報を使える
- `.dec` が存在しないユーザーモデルにも適用できる可能性がある

一方で、この方法はモデラー固有の式表現に依存する。そのため、OMMX 本体の decomposition データ形式は、`.dec` 由来か数式由来かに依存しない OMMX ID ベースの正規化済み表現にする。

最小構成では、分割情報は次のような内容を持つ。

```python
@dataclass
class Decomposition:
    blocks: dict[int, Block]
    master_constraint_ids: set[int]
    linking_variable_ids: set[int]
    source: str | None = None

@dataclass
class Block:
    id: int
    constraint_ids: set[int]
    variable_ids: set[int]
```

`.dec` が直接与えるのは主に constraint block である。variable block は、制約と変数の出現関係から推定する。数式レベルの検知では、constraint block に加えて、添字条件や separable expression の情報を使って block を構成できる場合がある。

- 1つの block の制約にだけ現れる変数は、その block の local variable
- 複数 block にまたがる変数は linking variable
- master constraints にだけ現れる変数は master-side variable として扱う

初期段階では、この情報を Python 側の構造体として持つ。必要になれば、後から OMMX Artifact の sidecar や protobuf message として永続化する。

重要なのは、入力経路ごとの差を `Decomposition` の外側に閉じ込めることである。`.dec` parser も数式レベルの検知器も、最終的には同じ `Decomposition` へ変換する。

## RMP

RMP はテンプレートとして構築できる。

column が以下の情報を持っていれば、RMP の構築は問題非依存にできる。

```python
@dataclass
class Column:
    block_id: int
    cost: float
    linking_activity: dict[int, float]
    state: dict[int, float]
    metadata: dict[str, str] = field(default_factory=dict)
```

RMP は column ごとに lambda 変数を持つ。

```text
min/max  sum_j cost_j lambda_j

s.t.     sum_j activity_ij lambda_j <= rhs_i      for linking/master constraints i
         sum_{j in block b} lambda_j == 1         for each block b
         lambda_j >= 0
```

OMMX 上では、各 lambda を continuous variable として作り、linking constraints と convexity constraints を通常の `Constraint` として追加する。

MVP では column を追加するたびに RMP の `Instance` を再構築し、`ommx-highs-adapter` で解く。性能が問題になった段階で、HiGHS model を直接増分更新する RMP backend を追加する。

## Pricing

pricing problem は問題固有なので、MVP ではユーザーが作成する。

典型的には、各 block に対して `ParametricInstance` を用意し、RMP の双対値を `Parameter` として渡す。

最小化問題の典型形は次のようになる。

```text
min  c_b x_b - sum_i pi_i A_{i,b} x_b - sigma_b
s.t. x_b in X_b
```

ここで `pi_i` は linking/master constraint の双対値、`sigma_b` は block の convexity constraint の双対値である。

ただし、実際の pricing の目的関数、符号、固定項、列の生成方法、整数変数の扱いは問題によって異なる。そのため、`ommx-column-generation` は pricing model を自動生成しない。

代わりに、次のような callback interface を提供する。

```python
class PricingSolver(Protocol):
    def solve_pricing(
        self,
        block_id: int,
        duals: PricingDuals,
    ) -> list[Column]:
        ...
```

`ommx-column-generation` は以下の補助を提供する。

- block ごとの subproblem 変数・制約の抽出
- dual parameter の作成補助
- linking activity の評価
- `Solution` から `Column` への変換補助
- reduced cost の判定補助

## MVP

最初の MVP では、性能より API と設計の検証を優先する。

MVP に含めるもの:

- `Column`
- `Decomposition`
- `RestrictedMasterProblem`
- RMP `Instance` の再構築
- `ommx-highs-adapter` による RMP solve
- RMP dual の取得
- pricing callback interface
- 小さい手作り問題の working example

MVP に含めないもの:

- branch-and-price
- HiGHS model の増分更新
- pricing problem の完全自動生成
- stabilization
- 高度な initial column generation
- infeasible RMP 用の本格的な Phase I

## 段階的な追加機能

### Step 1: 手作り example

小さい Dantzig-Wolfe 分解可能な LP/MIP を手で作り、decomposition も手で与える。

目的は RMP、dual、pricing callback、column 追加の最小ループを確認することである。

### Step 2: `.dec` loader

MIPLIB `.dec` を読み、OMMX constraint ID に変換する。

この段階では `.dec` は Python 側で処理してよい。OMMX 本体の schema 変更は避ける。

### Step 3: MIPLIB working example

MIPLIB の小さな decomposition instance を対象に、root LP column generation の working example を作る。

この時点で、OMMX 固有の分割情報に必要なフィールドを検証する。

### Step 4: Phase I

初期 column がない block や、RMP が infeasible になるケースに対応する。

最初は artificial column / artificial variable を使った単純な Phase I でよい。

### Step 5: RMP backend の改善

RMP を毎回 OMMX `Instance` として再構築する実装は単純だが遅い可能性がある。

必要になった段階で、HiGHS model に直接 column を追加する backend を導入する。

ただし、外部 API は `Column` と OMMX ID ベースのまま維持する。

### Step 6: 実験機能

必要に応じて以下を追加する。

- dual stabilization
- multiple columns per pricing
- heuristic pricing
- column pool pruning
- pricing solver の並列実行
- block ごとの solver 選択
- MIP pricing

## OMMX 本体との関係

このパッケージは、OMMX 本体に入れるべきデータ構造と、実験パッケージに置くべきアルゴリズムを分離するための検証場所である。

OMMX 本体に入れる候補:

- OMMX ID ベースの decomposition データ形式
- `.dec` から decomposition への変換
- decomposition の validation
- Artifact sidecar としての永続化

OMMX 本体に入れない候補:

- HiGHS 依存の RMP solver
- column generation の反復制御
- pricing callback framework
- branch-and-price
- 実験的な stabilization

`ommx-column-generation` で API が安定し、分割情報の必要フィールドが明確になってから、OMMX 本体に取り込むべき部分を判断する。

## Open Questions

- `.dec` から推定した variable block の仕様をどこまで SCIP/GCG に合わせるか
- master-only variables を linking variable と同じ扱いにするか、別 role にするか
- RMP の dual sign convention を OMMX の制約表現に合わせてどう固定するか
- maximization problem の扱いを内部で minimization に正規化するか
- `Column.state` は元の variable ID 全体を持つか、block local variable だけを持つか
- pricing が infeasible/unbounded を返した場合の扱い
- integer pricing を MVP に含めるか、LP pricing に限定するか
- 分割情報を metadata として保存する段階を挟むか、最初から専用 sidecar にするか

## まとめ

`ommx-column-generation` は、OMMX の分割情報設計を実際の column generation から検証するための実験的パッケージとして開始する。

RMP はテンプレートとして OMMX `Instance` に変換し、HiGHS adapter で解く。pricing は問題固有なので、ユーザー定義の callback として提供する。最初は working example として root LP column generation を実装し、性能改善や branch-and-price は後段に回す。

この方針により、OMMX 本体を重くせずに、MIPLIB `.dec` と Dantzig-Wolfe 分解の実用性を段階的に検証できる。
