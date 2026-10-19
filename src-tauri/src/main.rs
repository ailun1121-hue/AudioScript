#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::env::set_var("MKL_NUM_THREADS", "1");
    std::env::set_var("MKL_THREADING_LAYER", "SEQUENTIAL");
    std::env::set_var("OMP_NUM_THREADS", "1");
    app_lib::run();
}
