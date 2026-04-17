# Constraint ID 二重管理解消リファクタリング (WIP)

v3 開発中の大規模リファクタ。`Constraint<S>`, `IndicatorConstraint<S>`, `OneHotConstraint<S>`, `Sos1Constraint<S>` の 4 型から `id` フィールドを削除し、BTreeMap のキーを唯一の ID 保持場所とする。単独 PR の予定。

## 設計方針(確定済み)

### Rust コア
- 4 型から `pub id` フィールド削除
- コンストラクタから `id` 引数削除:
  - `Constraint::equal_to_zero(function)` / `less_than_or_equal_to_zero(function)`
  - `IndicatorConstraint::new(indicator_variable, equality, function)`
  - `OneHotConstraint::new(variables)`
  - `Sos1Constraint::new(variables)`
- `EvaluatedConstraintBehavior::constraint_id()` / `SampledConstraintBehavior::constraint_id()` 削除
- `From<Constraint> for v1::Constraint` → `From<(ConstraintID, Constraint)>`
- `Parse for v1::Constraint`: `Output = (ConstraintID, Constraint<Created>)`
- `Parse for v1::EvaluatedConstraint`: `Output = (ConstraintID, EvaluatedConstraint, Option<RemovedReason>)`
- `Parse for v1::SampledConstraint`: `Output = (ConstraintID, SampledConstraint, Option<RemovedReason>)`
- `Parse for v1::RemovedConstraint`: `Output = (ConstraintID, Constraint<Created>, RemovedReason)`
- `Instance::insert_constraint(id, constraint)`, `insert_constraints(Vec<(ID, Constraint)>)`
- `Constraint::to_bytes(&self, id: ConstraintID)`, `from_bytes(bytes) -> Result<(ID, Self)>`
- `InstanceError::Inconsistent*ID` 4 variants 削除 + 対応する validation ブロック削除
- `SolutionError::Inconsistent*ID` / `SampleSetError::Inconsistent*ID` 同様削除

### Python バインディング
- ラッパー型は `Constraint(pub ommx::Constraint)` のまま(ID なし、detached)
- `constraint.id` getter / `set_id()` / `next_constraint_id()` 系グローバル関数 / `CONSTRAINT_ID_COUNTER` **全廃止**
- `Instance.from_components(constraints={1: c, 2: c}, ...)` — **dict 入力**
- `instance.constraints` → `dict[int, Constraint]` — **dict 出力**
- 比較演算子 (`__eq__/__le__/__ge__`) は detached な Constraint を返す
- 対応ラッパー: `EvaluatedConstraint`, `SampledConstraint`, `IndicatorConstraint`, `OneHotConstraint`, `Sos1Constraint`, `RemovedConstraint`

### Adapter
`for c in instance.constraints:` → `for id, c in instance.constraints.items():`

---

## 完了した作業

### Rust コア — 型定義・trait・コンストラクタ
- [x] `rust/ommx/src/constraint.rs` — `Constraint<S>` struct から `id` 削除、コンストラクタ修正、`From<(ConstraintID, EvaluatedConstraint)> for v1::EvaluatedConstraint`、`From<(ConstraintID, SampledConstraint)> for v1::SampledConstraint`、ファイル内テスト修正
- [x] `rust/ommx/src/constraint/evaluate.rs` — Evaluate impl 内のリテラル修正
- [x] `rust/ommx/src/constraint/parse.rs` — Parse impl を tuple output に、ファイル内テスト修正
- [x] `rust/ommx/src/constraint/serialize.rs` — `to_bytes(&self, id)`, `from_bytes → Result<(ID, Self)>` に変更
- [x] `rust/ommx/src/constraint/arbitrary.rs` — placeholder 削除
- [x] `rust/ommx/src/constraint/logical_memory.rs` — `self.id` 参照削除、テスト修正(insta snapshot は要再生成)
- [x] `rust/ommx/src/constraint/reduce_binary_power.rs` — テスト内の struct literal 修正
- [x] `rust/ommx/src/constraint_type.rs` — trait から `constraint_id()` 削除、`ConstraintCollection::evaluate/evaluate_samples` を `iter()` ベースに、テスト修正
- [x] `rust/ommx/src/sos1_constraint/mod.rs` — `id` 削除、コンストラクタ、trait impl、テスト修正
- [x] `rust/ommx/src/sos1_constraint/evaluate.rs` — `self.id` 削除(error message も簡略化)、`check_sos1` ヘルパーから ID 引数除去、テストの `make_sos1` 修正
- [x] `rust/ommx/src/one_hot_constraint/mod.rs` — `id` 削除、コンストラクタ、trait impl、テスト修正
- [x] `rust/ommx/src/one_hot_constraint/evaluate.rs` — 同様修正
- [x] `rust/ommx/src/indicator_constraint/mod.rs` — `id` 削除、コンストラクタ、trait impl、テスト修正
- [x] `rust/ommx/src/indicator_constraint/evaluate.rs` — `self.id` 削除、Propagate 時の provenance 追加は caller の責務に(IndicatorPromote の metadata には provenance を入れない)、テスト修正(provenance テストは empty アサートに変更)

### Rust コア — MPS converter
- [x] `rust/ommx/src/mps/convert.rs` — 4 サイトの `equal_to_zero/less_than_or_equal_to_zero(id, f)` を単引数化

### Rust コア — Instance builder
- [x] `rust/ommx/src/instance/builder.rs` — 整合性 validation 5 ブロック(Constraint/RemovedConstraint/Indicator/RemovedIndicator/OneHot/Sos1)削除
- [x] `rust/ommx/src/instance/parametric_builder.rs` — 整合性 validation 2 ブロック削除

### Rust コア — Instance parse/serialize
- [x] `rust/ommx/src/instance/parse.rs` — `From<Constraint> for v1::Constraint` → `From<(ConstraintID, Constraint)>`、RemovedConstraint 同様、`From<Instance> for v1::Instance` の iterate を `(id, c)` ペア化、`instance_from_hints` 内の `OneHotConstraint::new` / `Sos1Constraint::new` 呼び出しを単引数化

### Rust コア — Solution/SampleSet 整合性検証
- [x] `rust/ommx/src/solution.rs` — `InconsistentConstraintID/IndicatorConstraintID/OneHotConstraintID/Sos1ConstraintID` の validation ブロック 4 つ削除、`ic.id` 等の参照を map iter 経由 `(id, ic)` で取るよう修正、`constraint.id` → `*constraint_id` に
- [x] `rust/ommx/src/sample_set.rs` — 同様の 4 つ削除

### Rust コア — Instance setter (API 変更)
- [x] `rust/ommx/src/instance/setter.rs` — `insert_constraint(id, constraint)` に API 変更、`insert_constraints(Vec<(ID, Constraint)>)` に変更、struct literal の `id: ...` 削除
  - ⚠️ **テスト関数内の呼び出し箇所 (line 195〜末尾) は未修正** — 20+ 箇所残る

---

## 残作業

### Rust コア

#### 1. `rust/ommx/src/instance/setter.rs` のテスト修正
**未完了箇所** (20+ サイト):
- `Constraint::equal_to_zero(ConstraintID::from(N), f)` → `Constraint::equal_to_zero(f)` 全て
- `instance.insert_constraint(constraint)` → `instance.insert_constraint(id, constraint)` (新 API)
- `instance.insert_constraints(vec![c1, c2])` → `instance.insert_constraints(vec![(id1, c1), (id2, c2)])`
- `assert_eq!(removed.id, new_constraint.id)` → ID を別途取得する形に変更
- `assert_eq!(instance.constraints().get(&constraint.id), Some(constraint))` の書き換え
- `btreemap! { ConstraintID::from(N) => Constraint::equal_to_zero(ConstraintID::from(N), f) }` → `btreemap! { ConstraintID::from(N) => Constraint::equal_to_zero(f) }`

#### 2. その他 instance/*.rs
- `rust/ommx/src/instance/new.rs` (line 88, 122, 123, 217): `Constraint::equal_to_zero(ConstraintID::from(N), f)` → 単引数
- `rust/ommx/src/instance/stats.rs` (line 214, 218, 311, 315, 319): 同様
- `rust/ommx/src/instance/analysis.rs` (line 323, 564, 571): `constraint.id` 参照 + コンストラクタ
- `rust/ommx/src/instance/slack.rs` (line 231, 232, 265, 266, 300, 301, 326, 327, 357, 358, 378, 379): コンストラクタ
- `rust/ommx/src/instance/qubo.rs` (line 153, 194): コンストラクタ
- `rust/ommx/src/instance/pass.rs` (line 156, 206, 255, 319, 320, 369, 370, 430, 431, 504, 505): Constraint literal `id:` 削除 + `IndicatorConstraint::new` の 4 引数→3 引数
- `rust/ommx/src/instance/penalty.rs` (line 278, 289): Constraint literal `id:` 削除
- `rust/ommx/src/instance/substitute.rs` (line 57, 80, 90, 136, 184, 232): `.id` 参照 + `IndicatorConstraint::new`
- `rust/ommx/src/instance/evaluate.rs` (line 274): Constraint literal `id:` 削除
- `rust/ommx/src/instance/logical_memory.rs` (line 245): `constraints.insert(constraint.id, constraint)` を呼び出し側に ID を渡す形に
- `rust/ommx/src/instance/parse.rs` テスト (line 528, 541, 589, 602, 619, 632, 663, 676, 849, 853, 862, 866, 892, 896, 905, 909): 全て `equal_to_zero(id, f)` 呼び出し

#### 3. `rust/ommx/src/solution.rs` テスト
- line 829: `constraint_id: constraint.id` → `*constraint_id` (修正済みの可能性あり、要確認)
- line 910, 921, 952, 962, 972, 1003, 1013, 1023, 1072, 1293, 1327: `Constraint::equal_to_zero/less_than_or_equal_to_zero(id, f)` 呼び出し
- line 1283: 同様
- line 1313: `SolutionError::InconsistentConstraintID { key, value_id }` 参照(エラー variant 削除と連動)

#### 4. `rust/ommx/src/solution/parse.rs`
- line 39-48: `let (parsed_constraint, removed_reason) = ec.parse_as(...)` を `let (id, parsed_constraint, removed_reason) = ...` に変更、`let id = parsed_constraint.id` 削除
- line 151: `v1::EvaluatedConstraint::from(ec.clone())` → `(id, ec.clone()).into()` ← 呼び出し側のコンテキスト要確認
- テスト内の EvaluatedConstraint 構築が `id:` を使っていれば削除

#### 5. `rust/ommx/src/sample_set/parse.rs`
- 同様のタプル化

#### 6. `rust/ommx/src/qplib/convert.rs`
- line 151, 165: `v1::Constraint { id: ... }` は v1 構造体なので id は残る(v1::Constraint は ID フィールド持ち)。ここは変更不要かも、要確認。

#### 7. `rust/ommx/src/mps/tests/edge_cases.rs`, `invalid_cases.rs`
- `Constraint::equal_to_zero(id, f)` → `Constraint::equal_to_zero(f)` (line 40, 93, 151, 155; line 41)

#### 8. `rust/ommx/src/lib.rs`
- doc コメント内のサンプルコード(line 75, 82, 134, 139, 206, 211): `Constraint::equal_to_zero(id, f)` → `Constraint::equal_to_zero(f)`

#### 9. `rust/ommx/examples/dependent_variables.rs` (line 45)
- コンストラクタ呼び出し修正

#### 10. `InstanceError` / `SolutionError` / `SampleSetError` の variants 削除
- `rust/ommx/src/instance/error.rs` (line 57, 63, 84, 90, 110, 116): `Inconsistent*ID` 6 variants 削除
- `rust/ommx/src/solution.rs` (line 81, 87, 93, 99): `Inconsistent*ID` 4 variants 削除
- `rust/ommx/src/sample_set.rs` (line 86, 92, 98, 104): `Inconsistent*ID` 4 variants 削除
- 関連テスト(`test_builder_inconsistent_*_id` 等)の削除

#### 11. `cargo check -p ommx`, `cargo test -p ommx` 通過
- insta snapshot 更新(`cargo insta review` または `cargo insta accept`)

---

### Python バインディング (8 ファイル)

#### 1. `python/ommx/src/constraint.rs`
- グローバルカウンター `CONSTRAINT_ID_COUNTER` 削除
- 関数 `next_constraint_id`, `set_constraint_id_counter`, `update_constraint_id_counter`, `get_constraint_id_counter` 削除(または非公開化)
- `Constraint::new()` から `id` 引数削除、内部の ID 生成削除
- `Constraint.id` getter 削除
- `Constraint::set_id()` 削除
- `Constraint` wrapper は現状の `pub struct Constraint(pub ommx::Constraint)` のまま
- `from_bytes` は `(ID, Constraint)` を返すようになったので、Python で ID 情報を捨てるか、別 API を用意するか。**決定: Python 側の to_bytes/from_bytes は ID なしとする(`Constraint::to_bytes(id=0)` 相当で serialize、`from_bytes` は ID を捨てる)**

#### 2. `python/ommx/src/evaluated_constraint.rs`
- `EvaluatedConstraint.id` getter 削除
- `to_bytes/from_bytes` 同様

#### 3. `python/ommx/src/sampled_constraint.rs`
- `SampledConstraint.id` getter 削除
- `to_bytes/from_bytes` 同様

#### 4. `python/ommx/src/indicator_constraint.rs`
- `new()` から id 引数削除(4 個)
- `id` getter 削除
- `set_id` 削除

#### 5. `python/ommx/src/one_hot_constraint.rs`
- 同様(`new()` から id 引数削除、`id` getter, `set_id` 削除)

#### 6. `python/ommx/src/sos1_constraint.rs`
- 同様

#### 7. `python/ommx/src/linear.rs`, `quadratic.rs`, `decision_variable.rs` の比較演算子
- `py_eq/py_le/py_ge` 内の `Constraint(ommx::Constraint { id, ... })` を `Constraint(ommx::Constraint::equal_to_zero(function))` 等に置換

#### 8. `python/ommx/src/parametric_instance.rs`
- `from_components` のシグネチャ変更(`constraints: Vec<Constraint>` → `constraints: BTreeMap<u64, Constraint>`)
- `get_constraint_by_id` の内部実装修正

#### 9. `python/ommx/src/instance.rs`
- `from_components` のシグネチャ変更(dict 入力)
- `constraints` プロパティ/メソッドを dict 返しに
- `indicator_constraints`, `one_hot_constraints`, `sos1_constraints` 同様

#### 10. `python/ommx/src/solution.rs`, `sample_set.rs`
- 評価済み constraint アクセスを dict に

---

### Python テスト (ommx-tests)

#### `python/ommx-tests/tests/`
- `test_constraint_wrapper.py`: `constraint.id` 参照、`Constraint(id=...)` 使用箇所 ~5 箇所書き換え
- `test_constraint_metadata.py`: 同様 ~5 箇所
- `test_removed_constraint_wrapper.py`: 同様
- `test_one_hot_sos1_constraints.py`: `instance.one_hot_constraints[0].id` → dict 経由
- `test_instance.py`: `removed.id` 参照
- `test_mps.py`: `b.id == a.id`

---

### Adapter (3 ファイル)

#### `python/ommx-highs-adapter/ommx_highs_adapter/adapter.py`
- line 480: `for constr in self.instance.constraints:` → `for id, constr in self.instance.constraints.items():`
- `constraint.id` 参照を id 変数に置き換え

#### `python/ommx-pyscipopt-adapter/ommx_pyscipopt_adapter/adapter.py`
- line 362 (sos1), 367 (constraints), 411 (indicator): 同様の `.items()` 化
- `constraint.id`, `sos1.id`, `indicator.id` 参照を id 変数に

#### `python/ommx-python-mip-adapter/ommx_python_mip_adapter/adapter.py`
- line 372: 同様

#### `python/ommx-openjij-adapter/ommx_openjij_adapter/__init__.py`
- line 266: `c.id for c in self.ommx_instance.constraints` → `cid for cid in self.ommx_instance.constraints.keys()` or `.items()` パターンに

---

### Stub 再生成

```bash
task python:stubgen
```

これで `python/ommx/_ommx_rust/__init__.pyi`, `python/ommx/v1/__init__.py` 更新。

---

### Docs

- `PYTHON_SDK_MIGRATION_GUIDE.md`: v3 alpha → v3 release で Constraint API が変わる旨追記
- `RUST_SDK_MIGRATION_GUIDE.md`: `Constraint::equal_to_zero(f)` 新 API、`insert_constraint(id, c)` 新 API、`Parse::Output` タプル化
- `docs/` のチュートリアル(en, ja)で `Constraint(id=...)`, `.set_id()`, `constraint.id` 使用箇所を書き換え

---

## 最終検証

```bash
task format
task rust:check
task rust:clippy
task rust:test
task python:stubgen
task python:test
task python:ommx-highs-adapter:test
task python:ommx-pyscipopt-adapter:test
task python:ommx-python-mip-adapter:test
task python:ommx-openjij-adapter:test
```

---

## メモ / 決定事項

- `Provenance::IndicatorConstraint(IndicatorConstraintID)` は**変更不要**(外部 ID を参照する別物)。ただし `IndicatorConstraint` の `propagate` 内で自身の ID を `Provenance` に追加していた処理は caller(Instance)の責務に移管済み — caller が `IndicatorConstraintID` を知っている場所で追加する必要あり。この点、`Instance` 側の propagate 呼び出し処理に provenance 追加コードを入れる必要がある(未完)
- `Sos1Constraint::evaluate`, `OneHotConstraint::evaluate`, `IndicatorConstraint::evaluate` の error message から ID を除去(ID を持たないため)
- `DecisionVariable`, `NamedFunction`, `Parameter` は**別 PR でやる**
- `to_bytes` は ID 引数必須にした(Rust)。Python 側は ID なし API に(to_bytes は ID=0 で serialize する実装 or 廃止)← 決定

## 見積もり

- 残り Rust core: 1.5〜2 時間(約 30 ファイル)
- Python: 1 時間(8 ファイル)
- Adapter + tests + docs: 30 分〜1 時間
- **合計: 3〜4 時間**
