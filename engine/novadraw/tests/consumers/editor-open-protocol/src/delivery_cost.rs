#[allow(dead_code, unused_imports)]
#[path = "../../../../crates/novadraw-editor/tests/p2_e03_container_editing_contract.rs"]
mod fixture;

fn main() {
    let result = fixture::delivery_cost_probe::<{ cfg!(feature = "container-cost") }>();
    println!("{result}");
}

/// Runs the same consumer through the packaged Wasm instance without platform stdout.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn run_container_probe() -> usize {
    fixture::delivery_cost_probe::<{ cfg!(feature = "container-cost") }>()
}
