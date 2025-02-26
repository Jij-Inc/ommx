//! # ランダム生成の例
//! 
//! このサンプルでは、OMMXのRust APIを使用してランダムな最適化問題を生成する方法を示します。

use ommx::v1::{Linear, Instance, State};
use ommx::random::{random_deterministic, LinearParameters, InstanceParameters};
use ommx::Evaluate;
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OMMXチュートリアル: ランダム生成 ===\n");

    // ランダムな線形関数の生成
    let linear_params = LinearParameters {
        num_terms: 5,
        max_id: 10,
    };
    let linear: Linear = random_deterministic(linear_params);
    println!("ランダムな線形関数: {:?}", linear);

    // 使用されている変数IDを取得
    let mut used_ids = HashSet::new();
    for term in &linear.terms {
        used_ids.insert(term.id);
    }
    println!("使用されている変数ID: {:?}", used_ids);

    // ランダムな最適化問題の生成
    let instance_params = InstanceParameters {
        num_constraints: 7,
        num_terms: 5,
        max_degree: 1,
        max_id: 10,
    };
    let instance: Instance = random_deterministic(instance_params);
    println!("\nランダムな最適化問題が生成されました:");
    println!("決定変数の数: {}", instance.decision_variables.len());
    println!("制約条件の数: {}", instance.constraints.len());
    println!("最適化の方向: {:?}", instance.sense);

    // ランダムな状態の生成（使用されている変数IDのみ）
    let mut state = State::default();
    for id in &used_ids {
        // 0.0から1.0の範囲のランダムな値を生成
        let random_value = (*id as f64 * 0.1) % 1.0; // 決定論的な値を使用
        state.entries.insert(*id, random_value * 2.0 - 1.0);
    }
    println!("\nランダムな状態: {:?}", state);

    // 線形関数の評価
    let (value, used_ids) = linear.evaluate(&state)?;
    println!("\n線形関数の評価結果: {}", value);
    println!("使用された変数ID: {:?}", used_ids);

    Ok(())
}
