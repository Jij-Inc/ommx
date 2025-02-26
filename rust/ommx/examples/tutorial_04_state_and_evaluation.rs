//! # 状態と評価の例
//!
//! このサンプルでは、OMMXのRust APIを使用して関数の評価と状態の操作を行う方法を示します。

use maplit::hashmap;
use ommx::v1::function::Function as FunctionEnum;
use ommx::v1::{Constraint, Equality, Function, Linear, State};
use ommx::Evaluate;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OMMXチュートリアル: 状態と評価 ===\n");

    // 線形関数の作成
    let linear = Linear::new([(1, 1.0), (2, 2.0), (3, 3.0)].into_iter(), 4.0);
    println!("線形関数: {:?}", linear);

    // 状態の作成（方法1: HashMapから）
    let state1: State = hashmap! { 1 => 2.0, 2 => 3.0, 3 => 4.0 }.into();
    println!("状態1: {:?}", state1);

    // 状態の作成（方法2: 直接構築）
    let mut state2 = State::default();
    state2.entries.insert(1, 2.0);
    state2.entries.insert(2, 3.0);
    state2.entries.insert(3, 4.0);
    println!("状態2: {:?}", state2);

    // 線形関数の評価
    let (value, used_ids) = linear.evaluate(&state1)?;
    println!("\n線形関数の評価結果: {}", value);
    println!("使用された変数ID: {:?}", used_ids);

    // 部分評価
    let mut linear_clone = linear.clone();
    let used_ids = linear_clone.partial_evaluate(&hashmap! { 1 => 2.0 }.into())?;
    println!("\n部分評価後の線形関数: {:?}", linear_clone);
    println!("使用された変数ID: {:?}", used_ids);

    // 制約条件の作成と評価
    let mut constraint = Constraint::default();
    constraint.id = 1;
    let mut function = Function::default();
    function.function = Some(FunctionEnum::Linear(linear.clone()));
    constraint.function = Some(function);
    constraint.equality = Equality::LessThanOrEqualToZero.into();
    constraint.name = Some("constraint1".to_string());

    let (evaluated_constraint, used_ids) = constraint.evaluate(&state1)?;
    println!("\n制約条件の評価結果: {:?}", evaluated_constraint);
    println!("使用された変数ID: {:?}", used_ids);
    println!(
        "制約条件は実行可能か: {}",
        evaluated_constraint.is_feasible(1e-6)?
    );

    Ok(())
}
