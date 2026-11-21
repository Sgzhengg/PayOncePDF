// 桌面入口：交给 lib 的 run()，避免 Windows 上无控制台时显示黑窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    payoncepdf_lib::run()
}
