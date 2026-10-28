#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent_catalog;
mod channels;
mod console_cli;
mod dispatch;
mod engine;
mod file_lock;
mod hosts;
mod model_find;
mod orchestration;
mod paths;
mod preset;
mod process;
mod recommend;
mod resources;
mod run_view;
mod server;
mod skill_install;
mod slash;
mod startup_env;

use anyhow::Result;

fn main() -> Result<()> {
    startup_env::prepare();

    // 带子命令时按命令行执行后退出；不带参数打开窗口。
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = console_cli::run_if_subcommand(&args) {
        std::process::exit(code);
    }
    // `aicross open <run_id>` 不短路：校验参数与 run 存在性后照常起窗口，
    // 请求随 AppState::pending_open 走；窗口已开时由单实例插件转发（server.rs）。
    // 打开动作只读，不派发、不改文件。退出码：0 已打开 / 2 参数错 /
    // 3 窗口起不来（引擎根不可用、其它读取错误同按 3）；只有 run 文件
    // 不存在按 1 找不到 run（fix-37-r1 R1-7）。
    let pending_open = match run_view::parse_open_args(&args) {
        run_view::OpenParse::NotOpen => None,
        run_view::OpenParse::Invalid(message) => {
            console_cli::attach_parent_console();
            eprintln!("{message}");
            std::process::exit(2);
        }
        run_view::OpenParse::Valid(run_id) => match paths::aicross_root() {
            Ok(root) => match run_view::open_run_exists(&root, &run_id) {
                Ok(true) => Some(run_id),
                Ok(false) => {
                    console_cli::attach_parent_console();
                    eprintln!("找不到 run：{run_id}");
                    std::process::exit(1);
                }
                Err(message) => {
                    console_cli::attach_parent_console();
                    eprintln!("{message}");
                    std::process::exit(3);
                }
            },
            Err(_) => {
                console_cli::attach_parent_console();
                eprintln!("引擎不可用");
                std::process::exit(3);
            }
        },
    };
    // R1-5（fix-37-r1）：宿主里调 `aicross open` 不能挂到窗口关闭——冷启动
    // 以分离方式另起窗口进程（argv 仍是 `open <run_id>`，被拉起的进程照常走
    // pending_open），自己立即退出 0；拉起失败按退出码 3。OPEN_WINDOW_ENV
    // 区分拉起方与被拉起方，避免无限接力。
    if let Some(run_id) = &pending_open {
        if std::env::var_os(run_view::OPEN_WINDOW_ENV).is_none() {
            match run_view::spawn_open_window(run_id) {
                Ok(()) => std::process::exit(0),
                Err(message) => {
                    console_cli::attach_parent_console();
                    eprintln!("{message}");
                    std::process::exit(3);
                }
            }
        } else {
            // 被拉起的窗口进程：清掉标记，不往下传。
            std::env::remove_var(run_view::OPEN_WINDOW_ENV);
        }
    }

    // 打开窗口（`open` 带来的直接进查看页）。冷启动 `aicross open` 起不来
    // 窗口时退出 3（走到这里的 open 只剩被拉起的窗口进程）。
    let open_requested = pending_open.is_some();
    match server::start_ui(pending_open) {
        Ok(()) => Ok(()),
        Err(error) => {
            if open_requested {
                console_cli::attach_parent_console();
                eprintln!("窗口启动失败：{error}");
                std::process::exit(3);
            }
            Err(error)
        }
    }
}
