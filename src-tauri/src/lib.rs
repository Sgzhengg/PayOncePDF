//! PayOnce PDF Tauri 2 后端入口：Builder + 插件注册 + generate_handler! + AppState。

pub mod commands;
pub mod engine;
pub mod error;
pub mod license;
pub mod logging;
pub mod state;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        // 注册插件。updater 的 pubkey 为占位符：插件启动期不解析 pubkey，
        // 仅 check() 时才用到，运行时若报错也不会影响其它命令，更不会 panic 整个 app。
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state::AppState::new())
        .setup(|app| {
            // 初始化日志目录（日志默认关闭，只写 时间/操作/错误码）。
            if let Ok(dir) = app.path().app_data_dir() {
                logging::init(dir);
            }
            // 首次启动建立 trial.json（7 天试用 + sha256 摘要）。
            let handle = app.handle().clone();
            let _ = license::get_license_state(&handle);
            // F21 无人值守自检：仅显式设置 PAYONCEPDF_SELFCHECK=1 时触发，
            // 普通启动完全不受影响。真机回归 / CI 可据此脚本化跑完自检，
            // 不必人工点击「运行自检」；报告另写 PAYONCEPDF_SELFCHECK_OUT 并以 .done 标记完成。
            if std::env::var("PAYONCEPDF_SELFCHECK").as_deref() == Ok("1") {
                let selfcheck_handle = app.handle().clone();
                std::thread::spawn(move || {
                    let report =
                        commands::selfcheck::run_self_check_blocking(&selfcheck_handle);
                    if let Ok(out) = std::env::var("PAYONCEPDF_SELFCHECK_OUT") {
                        let json = serde_json::to_string_pretty(&report).unwrap_or_default();
                        let _ = std::fs::write(&out, json);
                        let _ = std::fs::write(format!("{}.done", out), b"done");
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::selfcheck::run_self_check,
            commands::app::get_license_state,
            commands::app::activate_license,
            commands::app::cancel_job,
            commands::app::set_logging,
            commands::app::open_output_folder,
            commands::jobs::merge_pdfs,
            commands::jobs::split_pdf,
            commands::jobs::compress_pdf,
            commands::jobs::pdf_to_images,
            commands::jobs::images_to_pdf,
            commands::jobs::ocr_pdf,
            commands::jobs::sign_pdf,
            commands::jobs::batch_run,
            commands::extras::replace_text,
            commands::extras::edit_text_at_rect,
            commands::extras::annotate_pdf,
            commands::extras::manage_pages,
            commands::extras::decorate_pdf,
            commands::extras::export_pdf,
            commands::objects::create_form_pdf,
            commands::objects::redact_pdf,
            commands::objects::list_bookmarks,
            commands::objects::manage_bookmarks,
            commands::compare::compare_documents,
            commands::compare::export_compare_report,
            commands::more::text_to_pdf,
            commands::more::share_file,
            commands::more::list_ocr_languages,
            commands::more::install_ocr_language,
            commands::viewer::get_pdf_info,
            commands::viewer::render_page_preview,
            commands::viewer::get_page_text_layout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
