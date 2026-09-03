use novadraw_apps::{
    run_demo_app, run_demo_app_with_scene_screenshot, run_demo_app_with_screenshot,
};
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let title = "Shape App";
    let app_name = "shape-app";
    let scenes = novadraw_demo_scenes::shape::scenes();

    if args.len() > 1 {
        match args[1].as_str() {
            "--screenshot-all" => {
                println!("截图所有场景...");
                std::io::stdout().flush().ok();
                run_demo_app_with_screenshot(title, app_name, scenes, true)
                    .expect("Failed to run app with screenshot");
            }
            arg if arg.starts_with("--screenshot=") => {
                let scene_idx = arg
                    .strip_prefix("--screenshot=")
                    .and_then(|value| value.parse::<usize>().ok());
                match scene_idx {
                    Some(index) => {
                        run_demo_app_with_scene_screenshot(title, app_name, scenes, index)
                            .expect("Failed to run app with scene screenshot")
                    }
                    None => {
                        eprintln!("无效的场景索引: {}", &arg[12..]);
                        eprintln!("用法: cargo run -- --screenshot=<0-9> 或 --screenshot-all");
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                println!("用法: cargo run -- [选项]");
                println!("  --screenshot-all    截图所有场景");
                println!("  --screenshot=<N>   截图指定场景 (0-9)");
            }
            _ => {
                eprintln!("未知参数: {}", args[1]);
                std::process::exit(1);
            }
        }
    } else {
        run_demo_app(title, app_name, scenes).expect("Failed to run app");
    }
}
