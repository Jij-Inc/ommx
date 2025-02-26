//! # アーティファクトの例
//! 
//! このサンプルでは、OMMXのRust APIを使用してアーティファクトを作成し、操作する方法を示します。

use anyhow::Result;
use ocipkg::ImageName;
use ommx::{
    artifact::{Builder, InstanceAnnotations},
    random::{random_deterministic, InstanceParameters},
    v1::Instance,
};
use url::Url;
use std::path::PathBuf;

fn main() -> Result<()> {
    println!("=== OMMXチュートリアル: アーティファクト ===\n");

    // ランダムな最適化問題の生成
    let instance: Instance = random_deterministic(InstanceParameters {
        num_constraints: 5,
        num_terms: 3,
        max_degree: 1,
        max_id: 5,
    });
    println!("ランダムな最適化問題が生成されました:");
    println!("決定変数の数: {}", instance.decision_variables.len());
    println!("制約条件の数: {}", instance.constraints.len());

    // 一時ファイルパスの作成
    let artifact_path = PathBuf::from("/tmp/example_instance.ommx");
    println!("\nアーティファクトの保存先: {}", artifact_path.display());

    // アーティファクトの作成
    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/example_instance:latest")?;
    println!("イメージ名: {}", image_name);

    // アノテーションの設定
    let mut annotations = InstanceAnnotations::default();
    annotations.set_title("Example Instance".to_string());
    annotations.set_created(chrono::Local::now());

    // ビルダーの作成とインスタンスの追加
    let mut builder = Builder::new_archive(artifact_path.clone(), image_name)?;
    builder.add_instance(instance, annotations)?;
    builder.add_source(&Url::parse("https://github.com/Jij-Inc/ommx")?);
    builder.add_description("OMMXチュートリアル用のサンプルアーティファクト".to_string());
    
    // アーティファクトのビルド
    let _artifact = builder.build()?;
    println!("\nアーティファクトが正常に作成されました: {}", artifact_path.display());

    // アーティファクトからの読み込み
    // 注意: 実際のアプリケーションでは、ここでアーティファクトを読み込んで使用します
    println!("\nアーティファクトからインスタンスを読み込むには、以下のようなコードを使用します:");
    println!(r#"
    use ommx::artifact::Artifact;
    
    let artifact = Artifact::open_archive("/path/to/artifact.ommx")?;
    let instance = artifact.get_instance()?;
    println!("読み込まれたインスタンス: {{}}", instance);
    "#);

    Ok(())
}
