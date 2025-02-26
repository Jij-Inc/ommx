//! # 線形関数の例
//! 
//! このサンプルでは、OMMXのRust APIを使用して線形関数を作成し、操作する方法を示します。

use ommx::v1::{Linear, State};
use ommx::Evaluate;
use prost::Message;
use maplit::hashmap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OMMXチュートリアル: 線形関数 ===\n");

    // 方法1: 単一の項から線形関数を作成
    let linear1 = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
    println!("線形関数1: {:?}", linear1);

    // 方法2: イテレータから線形関数を作成
    let linear2 = Linear::new([(1, 1.0), (2, 2.0)].into_iter(), 3.0);
    println!("線形関数2: {:?}", linear2);

    // 方法3: 空の線形関数から開始し、項を追加
    let mut linear3 = Linear::default();
    linear3.constant = 3.0;
    // 注意: Termは直接作成できないため、Linear::single_termを使用して追加
    linear3 = linear3 + Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0);
    println!("線形関数3: {:?}", linear3);

    // 線形関数の評価
    let state: State = hashmap! { 1 => 2.0, 2 => 3.0 }.into();
    
    let (value, used_ids) = linear1.evaluate(&state)?;
    println!("\n状態 x_1 = 2.0, x_2 = 3.0 での線形関数の値: {}", value);
    println!("使用された変数ID: {:?}", used_ids);

    // 線形関数の操作
    let linear4 = linear1.clone() + linear2.clone();
    println!("\n線形関数1 + 線形関数2: {:?}", linear4);

    let linear5 = linear1.clone() * 2.0;
    println!("線形関数1 * 2.0: {:?}", linear5);

    // シリアライズとデシリアライズ
    let mut buf = Vec::new();
    linear1.encode(&mut buf)?;
    
    let decoded_linear = Linear::decode(buf.as_slice())?;
    println!("\nデシリアライズされた線形関数: {:?}", decoded_linear);

    Ok(())
}
