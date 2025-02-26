//! # 二次関数の例
//! 
//! このサンプルでは、OMMXのRust APIを使用して二次関数を作成し、操作する方法を示します。

use ommx::v1::{Linear, Quadratic, State};
use ommx::Evaluate;
use prost::Message;
use maplit::hashmap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OMMXチュートリアル: 二次関数 ===\n");

    // 線形関数の作成
    let linear1 = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
    println!("線形関数: {:?}", linear1);

    // 方法1: 線形関数から二次関数を作成
    let mut quadratic1 = Quadratic::default();
    quadratic1.linear = Some(linear1.clone());
    println!("二次関数1 (線形部分のみ): {:?}", quadratic1);

    // 方法2: 二次項を持つ二次関数を作成
    let mut quadratic2 = Quadratic::default();
    quadratic2.linear = Some(linear1.clone());
    quadratic2.rows = vec![1, 1, 2];
    quadratic2.columns = vec![1, 2, 2];
    quadratic2.values = vec![1.0, 2.0, 3.0];
    println!("二次関数2 (二次項あり): {:?}", quadratic2);

    // 方法3: 線形関数の積から二次関数を作成
    let linear2 = Linear::single_term(1, 1.0) + Linear::single_term(2, 1.0);
    let quadratic3 = linear1.clone() * linear2;
    println!("二次関数3 (線形関数の積): {:?}", quadratic3);

    // 二次関数の評価
    let state: State = hashmap! { 1 => 2.0, 2 => 3.0 }.into();
    
    let (value, used_ids) = quadratic2.evaluate(&state)?;
    println!("\n状態 x_1 = 2.0, x_2 = 3.0 での二次関数の値: {}", value);
    println!("使用された変数ID: {:?}", used_ids);

    // 二次関数の操作
    if let Some(linear) = quadratic2.linear.clone() {
        quadratic2.linear = Some(linear * 2.0);
    }
    println!("\n線形部分を2倍した二次関数: {:?}", quadratic2);

    // シリアライズとデシリアライズ
    let mut buf = Vec::new();
    quadratic2.encode(&mut buf)?;
    
    let decoded_quadratic = Quadratic::decode(buf.as_slice())?;
    println!("\nデシリアライズされた二次関数: {:?}", decoded_quadratic);

    Ok(())
}
