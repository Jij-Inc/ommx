# OMMX Column Generation Proposal

## 背景

OMMX で MIPLIB の `.dec` ファイルを扱えるようにすると、MIPLIB が提供する分割情報を OMMX の内部 ID に対応付けられる。これにより、単に分割情報を保存するだけでなく、Dantzig-Wolfe 分解、column generation、decomposition-aware heuristics などの実装に利用できる。

一方で、Dantzig-Wolfe 分解を OMMX 本体に直接入れるのは現時点では重い。Restricted Master Problem (RMP) は LP として HiGHS で解くのが自然であり、HiGHS 依存を OMMX core に入れるべきではない。また、pricing problem は問題ごとに構造が大きく異なるため、完全に汎用化するには時間がかかる。

そのため、まずは OMMX 本体とは別の実験的 Python パッケージとして `ommx-column-generation` を作り、working example から始める方針を取る。

## 目的

この提案の目的は、OMMX における column generation の最小コア API を定め、その前段にある分割情報や高次モデラー連携を、実際に使う側から逆算して設計することである。ただし、分割情報は column generation 専用の入力ではなく、heuristics や他の分解ベース手法でも共有する OMMX の共通構造情報として扱う。

最初から高性能な汎用 Dantzig-Wolfe solver を目指すのではなく、以下を満たす最小構成を目指す。

- 手作りの RMP 行と column から RMP を構築できる
- RMP を `ommx-highs-adapter` で解ける
- RMP の双対値を `PricingOracle` に渡せる
- pricing 側はユーザーが自由に実装できる
- pricing 解から column を生成し、RMP に追加できる
- 将来的に OMMX の `Instance` と分割情報から RMP / pricing を自動生成できる
- 分割情報を column generation 以外の heuristics や他手法でも再利用できる
- `ParametricInstance` を使う pricer を、`PricingOracle` の実装方法の1つとして扱える
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

- core API としての column generation の反復制御
- RMP 行と column からの RMP 構築
- RMP の解と双対値の取得
- `PricingOracle` との接続
- column pool の管理
- 実験的な `.dec` loader / decomposition helper
- 実験的な `ParametricInstance` pricer helper

HiGHS 依存や column generation の実験的 API は OMMX 本体に入れない。

## Core API

column generation の中心は、分割情報そのものではなく、RMP と pricer をつなぐ最小標準形である。

MVP の core API は次の形を取る。

```python
@dataclass(frozen=True)
class MasterRow:
    id: Hashable
    sense: Literal["<=", ">=", "=="]
    rhs: float
    name: str | None = None

@dataclass(frozen=True)
class Column:
    id: Hashable
    cost: float
    coefficients: Mapping[Hashable, float]
    payload: Any = None

@dataclass
class ColumnGenerationProblem:
    rows: list[MasterRow]
    columns: list[Column]
    sense: Literal["minimize", "maximize"] = "minimize"
```

`MasterRow` は RMP の行を表し、`Column` はその行 ID に対する係数と目的関数係数を持つ。core はこの情報から RMP の OMMX `Instance` を構築し、HiGHS adapter などの master solver で解く。

pricing 側の標準境界は `ParametricInstance` ではなく `PricingOracle` である。

```python
class PricingOracle(Protocol):
    def __call__(self, context: PricingContext) -> PricingResult:
        ...
```

`PricingContext` は RMP 解、RMP 行、既存 column、dual、tolerance を持つ。`PricingOracle` はそれを受け取り、新しい `Column` を返す。core は pricing の内部表現を知らない。

この境界を core にする理由は、pricing problem が必ずしも OMMX `ParametricInstance` として表現されるとは限らないためである。問題によっては、dynamic programming、shortest path、knapsack 専用 solver、アニーリング、heuristic などを直接呼び出す方が自然である。

したがって、このプロジェクトで column generation を扱う各ルートは、最終的に次の形へ落ちる。

```text
RMP rows + initial columns + PricingOracle
  -> RMP Instance を構築
  -> RMP solve
  -> dual を PricingOracle へ渡す
  -> pricing result から Column を追加
```

分割情報、`.dec` loader、高次モデラー連携は、この core API の一段前で RMP 行、初期 column、`PricingOracle` を構成するための builder / helper layer である。元問題や高次モデラーから来る標準ルートでは、OMMX の分割情報に正規化してから RMP / Pricer を構成する。これは分割情報を column generation 以外の用途にも使えるようにするためである。

```text
.dec + flat OMMX Instance
  -> Decomposition
  -> column generation builder/helper
  -> RMP rows + columns + PricingOracle
  -> core loop

高次モデラー上の分解記述 (標準ルート)
  -> Decomposition
  -> column generation builder/helper
  -> RMP rows + columns + PricingOracle
  -> core loop

高次モデラー上の分解記述 (直接 adapter)
  -> direct column generation adapter
  -> RMP rows + columns + PricingOracle
  -> core loop

手作り RMP / Pricer
  -> RMP rows + columns + PricingOracle
  -> core loop
```

`ParametricInstance` はこの builder / helper layer で重要になる。OMMX モデルとして pricing を書ける場合には、次の情報をまとめた adapter を用意すれば、`PricingOracle` として core に接続できる。

```text
pricing ParametricInstance
+ RMP row ID から dual parameter への対応
+ pricing Solution から Column への変換
+ reduced cost / termination の判定
  -> PricingOracle
```

つまり、`ParametricInstance` pricer は標準的な実装候補だが、core API の必須入力ではない。

core API で固定すべき標準形は次である。

- RMP 行は安定した `MasterRow.id` を持つ
- `Column.coefficients` は `MasterRow.id` に対する係数を持つ
- RMP の dual は元の `MasterRow.sense` に対する向きで `PricingOracle` に渡す
- `PricingOracle` は負の reduced cost を持つ column 候補を返す
- 厳密 pricing で改善 column が存在しないことを証明した場合は `proven_no_negative_reduced_cost=True` を返す
- heuristic pricing は column を返してよいが、改善不能の証明としては扱わない
- pricing 解、block ID、元変数値、モデラー固有情報は `Column.payload` に入れ、core は解釈しない

## 分割情報

分割情報には複数の入力経路があり得る。

1つは MIPLIB の `.dec` ファイルである。`.dec` は入力フォーマットとして扱い、OMMX 内では OMMX 固有の分割情報に正規化する。

`.dec` は制約名ベースの形式なので、ロード時に MPS の row name と照合し、OMMX の constraint ID に変換する。変換後は MPS 名に依存しない。

もう1つの経路は、JijModeling のような数式レベルのモデラーから構造情報を得る方法である。モデラーは係数行列に展開される前の添字、`forall`、条件式、和の構造を保持しているため、問題によっては巨大な係数情報を構築せずに、数式構造だけから separable constraints と overlapped constraints を検知できる。

この経路には以下の利点がある。

- 巨大な MPS / matrix を作る前に分割構造を検知できる
- 添字や条件式に由来する block 構造を失わずに扱える
- モデル生成時点の意味情報を使える
- `.dec` が存在しないユーザーモデルにも適用できる可能性がある

一方で、この方法はモデラー固有の式表現に依存する。そのため、OMMX 本体の decomposition データ形式は、`.dec` 由来か数式由来かに依存しない OMMX ID ベースの正規化済み表現にする。高次モデラー連携では、column generation に特化した RMP / Pricer を直接生成するルートも排除しないが、それだけを標準ルートにはしない。共通構造として再利用できる場合は、この正規化済み分割情報を生成する。

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

重要なのは、入力経路ごとの差を core column generation loop から分離することである。`.dec` parser や数式レベルの検知器は、共通利用したい構造を同じ `Decomposition` へ変換する。column generation 用の RMP / Pricer は、その `Decomposition` を入力にして構成できる。一方で、特定の高次モデラーや特定の pricing 実装が直接 RMP / Pricer を構成するルートも、core API への adapter として許容する。

### Column generation 以外での用途

分割情報は column generation のためだけのメタデータではない。OMMX の問題構造を表す共通データとして、heuristics や他の分解ベース手法でも利用する。

想定する用途は次の通り。

- block ごとの local search
- decomposition-aware large neighborhood search
- relax-and-fix / fix-and-optimize
- block coordinate descent 的な heuristic
- block ごとの solver 選択
- linking variables を境界とする neighborhood 生成
- block subproblem の並列処理
- 問題構造の可視化や診断

このため、`.dec` や高次モデラーから得た構造情報を column generation 専用の RMP / Pricer へ直接変換して終わりにする設計だけでは不十分である。共通利用できる構造は OMMX 固有の `Decomposition` として保持し、その上に column generation builder や heuristic builder を載せる。

```text
.dec / 高次モデラー / 手動分割指定
  -> OMMX Decomposition
      -> column generation builder
      -> heuristic builder
      -> visualization / diagnostics

高次モデラー固有の直接変換
  -> RMP / Pricer
  -> column generation core
```

### 分割情報と RMP / Pricer の関係

分割情報は column generation の反復そのものに必須ではない。ユーザーが問題を手計算で RMP と pricing problem に変換し、RMP 行、初期 column、`PricingOracle` を直接与える場合、`ommx-column-generation` は分割情報なしで動作できる。

この場合、OMMX が扱うのはすでに変換済みの構造である。

```text
手作り RMP rows + initial columns + PricingOracle
  -> column generation loop
```

一方、元の OMMX `Instance` や高次モデラーの構造情報から RMP や pricing problem を機械的に生成したい場合には、標準的には分割情報を共通中間表現として使う。分割情報があれば、各 block の local variables、local constraints、master / linking constraints、目的関数と linking 制約への寄与を切り出し、pricing 用の `ParametricInstance` や `PricingOracle` を構成できる。

```text
元 Instance / 高次モデラー
  -> Decomposition
  -> RMP template
  -> block ごとの pricing ParametricInstance / PricingOracle
  -> column generation loop
```

つまり、分割情報は「元問題から RMP / pricing への変換を自動化するためのメタデータ」であり、「手作り済みの RMP / Pricer を実行するための必須入力」ではない。

### `.dec` ルートと高次モデラールート

`.dec` を使う場合、入力はすでに MPS / OMMX `Instance` のような低次表現に展開されている。そのため、`.dec` parser は row name や constraint ID をもとに block を復元し、係数行列レベルの情報から RMP / pricing を生成する。

このルートは MIPLIB のような既存 benchmark には有効だが、得られる情報は基本的に平坦化された変数・制約・係数である。pricing が knapsack、shortest path、assignment、matching、QUBO / PUBO、dynamic programming で解ける構造などを持つ場合、その特殊構造を係数行列から復元するのは難しいことがある。

一方、JijModeling のような高次のモデラーがある場合は、分割情報の検出を数式レベルで行う方が自然である。モデラー上では添字集合、条件式、集約、`forall`、separable expression などが残っているため、ユーザーやモデラー拡張が「この添字ごとに block を作る」「この式は block local である」といった構造を直接記述できる。

この場合の標準ルートでは、OMMX 固有の `Decomposition` を生成する。column generation ではその `Decomposition` から RMP 行、column、`PricingOracle` を構成する。pricing problem の形が既知である場合は、モデラー由来の追加情報を `Decomposition` に紐付け、`ParametricInstance` や専用 solver adapter を作るために使ってよい。高次モデラーから RMP / Pricer へ直接変換するルートも排除しないが、これは column generation 専用の adapter であり、分割情報の共通利用を置き換えるものではない。

```text
高次モデラー上の分解記述
  -> OMMX Decomposition
  -> column generation builder
  -> RMP rows / columns
  -> ParametricInstance または専用 solver を使う PricingOracle
  -> column generation loop
```

このルートは、`.dec` よりもモデラー固有である一方、表現範囲が広い。特に pricing problem の形が既知で、その構造を保ったまま専用 solver やアニーリング、動的計画法などに渡したい場合に扱いやすい。

## RMP

RMP はテンプレートとして構築できる。

column が以下の情報を持っていれば、RMP の構築は問題非依存にできる。

```python
@dataclass
class Column:
    id: Hashable
    cost: float
    coefficients: Mapping[Hashable, float]
    payload: Any = None
```

`coefficients` は `MasterRow.id` を key とする。`payload` には、pricing 解、元変数の値、block ID、モデラー固有情報など、core loop が解釈しない情報を保持できる。

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

pricing problem は問題固有なので、MVP ではユーザーが `PricingOracle` として作成する。

典型的には、各 block に対して `ParametricInstance` を用意し、RMP の双対値を `Parameter` として渡す。ただし pricing は OMMX `ParametricInstance` に限定しない。問題によっては、専用の組合せ最適化 solver、heuristic、アニーリング、dynamic programming などを `PricingOracle` の中で呼び出す方が自然である。

最小化問題の典型形は次のようになる。

```text
min  c_b x_b - sum_i pi_i A_{i,b} x_b - sigma_b
s.t. x_b in X_b
```

ここで `pi_i` は linking/master constraint の双対値、`sigma_b` は block の convexity constraint の双対値である。

ただし、実際の pricing の目的関数、符号、固定項、列の生成方法、整数変数の扱いは問題によって異なる。そのため、MVP の `ommx-column-generation` は pricing model を自動生成せず、`PricingOracle` の境界で受け取る。

代わりに、次のような `PricingOracle` interface を提供する。

```python
class PricingOracle(Protocol):
    def __call__(self, context: PricingContext) -> PricingResult:
        ...
```

将来的に `ParametricInstance` pricer helper は以下の補助を提供する。

- block ごとの subproblem 変数・制約の抽出
- dual parameter の作成補助
- linking activity の評価
- `Solution` から `Column` への変換補助
- reduced cost の判定補助

これらの補助は、`.dec` 由来または高次モデラー由来の `Decomposition` から pricing `ParametricInstance` を自動生成する段階で重要になる。一方、専用 solver を直接使う場合は、`Decomposition` と必要なモデラー由来メタデータを参照して `PricingOracle` を実装してもよい。

## MVP

最初の MVP では、性能より API と設計の検証を優先する。

MVP に含めるもの:

- `MasterRow`
- `Column`
- `ColumnGenerationProblem`
- `RestrictedMasterProblem`
- RMP `Instance` の再構築
- `ommx-highs-adapter` による RMP solve
- RMP dual の取得
- `PricingOracle` interface
- 小さい手作り問題の working example

MVP に含めないもの:

- branch-and-price
- HiGHS model の増分更新
- pricing problem の完全自動生成
- `ParametricInstance` pricer helper
- 元 `Instance` と `Decomposition` からの RMP / pricing 自動生成
- stabilization
- 高度な initial column generation
- infeasible RMP 用の本格的な Phase I

## 段階的な追加機能

### Step 1: 手作り example

小さい Dantzig-Wolfe 分解可能な LP/MIP を手で RMP と `PricingOracle` に変換する。

目的は RMP、dual、`PricingOracle`、column 追加の最小ループを確認することである。

### Step 2: `.dec` loader

MIPLIB `.dec` を読み、OMMX constraint ID に変換する。

この段階では `.dec` は Python 側で処理してよい。OMMX 本体の schema 変更は避ける。

### Step 3: Decomposition からの自動生成

OMMX `Instance` と `Decomposition` から、RMP rows、初期 columns、block ごとの `PricingOracle` を生成する。pricing を OMMX で表現できる場合は、block ごとの pricing `ParametricInstance` とその adapter を生成する。

この段階で、分割情報に必要なフィールドを検証する。特に、local variable、linking variable、master constraint、block objective contribution、linking activity の扱いを明確にする。

### Step 4: MIPLIB working example

MIPLIB の小さな decomposition instance を対象に、root LP column generation の working example を作る。

この時点で、OMMX 固有の分割情報に必要なフィールドを検証する。

### Step 5: 高次モデラー連携

高次モデラー上の数式構造から OMMX `Decomposition` を生成し、それを `ommx-column-generation` の builder に接続する working example を作る。

このルートでは、数式レベルの構造を使って分割情報を作り、その分割情報を column generation と heuristics の両方で再利用できることを確認する。pricing で必要な追加の数式情報は、`Decomposition` に紐付くメタデータや adapter 側の情報として保持する。必要に応じて高次モデラーから RMP / Pricer へ直接接続する adapter も許容するが、これは共通の `Decomposition` ルートを補完するものと位置づける。

### Step 6: Phase I

初期 column がない block や、RMP が infeasible になるケースに対応する。

最初は artificial column / artificial variable を使った単純な Phase I でよい。

### Step 7: RMP backend の改善

RMP を毎回 OMMX `Instance` として再構築する実装は単純だが遅い可能性がある。

必要になった段階で、HiGHS model に直接 column を追加する backend を導入する。

ただし、外部 API は `Column` と OMMX ID ベースのまま維持する。

### Step 8: 実験機能

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
- heuristics や他手法でも使える decomposition metadata の共通 schema

OMMX 本体に入れない候補:

- HiGHS 依存の RMP solver
- column generation の反復制御
- `PricingOracle` framework
- branch-and-price
- 実験的な stabilization

`ommx-column-generation` で API が安定し、分割情報の必要フィールドが明確になってから、OMMX 本体に取り込むべき部分を判断する。

## Open Questions

- `.dec` から推定した variable block の仕様をどこまで SCIP/GCG に合わせるか
- master-only variables を linking variable と同じ扱いにするか、別 role にするか
- RMP の dual sign convention を OMMX の制約表現に合わせてどう固定するか
- maximization problem の扱いを内部で minimization に正規化するか
- `Column.payload` に元の variable ID 全体を持つか、block local variable だけを持つか
- pricing が infeasible/unbounded を返した場合の扱い
- integer pricing を MVP に含めるか、LP pricing に限定するか
- 分割情報を metadata として保存する段階を挟むか、最初から専用 sidecar にするか
- heuristics でも共通利用するために `Decomposition` にどこまでの情報を持たせるか

## まとめ

`ommx-column-generation` は、OMMX における column generation の core API と、その前段にある分割情報・高次モデラー連携を検証するための実験的パッケージとして開始する。ただし、分割情報は column generation だけのために設計せず、heuristics や他の分解ベース手法にも使える共通構造情報として設計する。

RMP はテンプレートとして OMMX `Instance` に変換し、HiGHS adapter で解く。pricing は問題固有なので、ユーザー定義の `PricingOracle` として提供する。`ParametricInstance` pricer はその実装方法の1つとして扱い、core API の必須入力にはしない。最初は working example として root LP column generation を実装し、性能改善や branch-and-price は後段に回す。

この方針により、OMMX 本体を重くせずに、MIPLIB `.dec`、高次モデラー由来の分割構造、Dantzig-Wolfe 分解、decomposition-aware heuristics の実用性を段階的に検証できる。
