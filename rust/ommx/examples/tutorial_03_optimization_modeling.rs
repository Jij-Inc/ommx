//! # 最適化問題のモデリング例
//! 
//! このサンプルでは、OMMXのRust APIを使用して最適化問題をモデリングする方法を示します。

use ommx::v1::{
    Bound, Constraint, DecisionVariable, Equality, Function, Instance, Linear,
    decision_variable::Kind,
    function::Function as FunctionEnum,
    instance::Sense,
};
use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OMMXチュートリアル: 最適化問題のモデリング ===\n");

    // 決定変数の作成
    let mut x1 = DecisionVariable::default();
    x1.id = 1;
    x1.name = Some("x1".to_string());
    x1.kind = Kind::Binary.into();

    let mut x2 = DecisionVariable::default();
    x2.id = 2;
    x2.name = Some("x2".to_string());
    x2.kind = Kind::Integer.into();
    let mut bound2 = Bound::default();
    bound2.lower = -10.0;
    bound2.upper = 10.0;
    x2.bound = Some(bound2);

    let mut x3 = DecisionVariable::default();
    x3.id = 3;
    x3.name = Some("x3".to_string());
    x3.kind = Kind::Continuous.into();
    let mut bound3 = Bound::default();
    bound3.lower = 0.0;
    bound3.upper = f64::INFINITY; // 上限なし
    x3.bound = Some(bound3);

    // 目的関数の作成
    let objective_linear = Linear::new([(1, 1.0), (2, 2.0), (3, 3.0)].into_iter(), 0.0);
    let mut objective = Function::default();
    objective.function = Some(FunctionEnum::Linear(objective_linear));

    // 制約条件の作成
    let constraint1_linear = Linear::new([(1, 1.0), (2, 1.0)].into_iter(), -5.0);
    let mut constraint1 = Constraint::default();
    constraint1.id = 1;
    let mut function1 = Function::default();
    function1.function = Some(FunctionEnum::Linear(constraint1_linear));
    constraint1.function = Some(function1);
    constraint1.equality = Equality::LessThanOrEqualToZero.into();
    constraint1.name = Some("constraint1".to_string());

    let constraint2_linear = Linear::new([(2, 1.0), (3, 2.0)].into_iter(), -8.0);
    let mut constraint2 = Constraint::default();
    constraint2.id = 2;
    let mut function2 = Function::default();
    function2.function = Some(FunctionEnum::Linear(constraint2_linear));
    constraint2.function = Some(function2);
    constraint2.equality = Equality::EqualToZero.into();
    constraint2.name = Some("constraint2".to_string());

    // インスタンスの作成
    let mut instance = Instance::default();
    instance.decision_variables = vec![x1, x2, x3];
    instance.constraints = vec![constraint1, constraint2];
    instance.objective = Some(objective);
    instance.sense = Sense::Minimize.into();

    println!("最適化問題が作成されました:");
    println!("決定変数の数: {}", instance.decision_variables.len());
    println!("制約条件の数: {}", instance.constraints.len());
    println!("最適化の方向: {:?}", instance.sense);

    // シリアライズとデシリアライズ
    let mut buf = Vec::new();
    instance.encode(&mut buf)?;
    
    let decoded_instance = Instance::decode(buf.as_slice())?;
    println!("\nデシリアライズされた最適化問題:");
    println!("決定変数の数: {}", decoded_instance.decision_variables.len());
    println!("制約条件の数: {}", decoded_instance.constraints.len());
    println!("最適化の方向: {:?}", decoded_instance.sense);

    Ok(())
}
