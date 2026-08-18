use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use clap::Parser;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "Knot Launcher - Run any single-seat app inside a Knot Island")]
struct Args {
    #[arg(long, default_value = "wayland-knot-0", help = "Parent knot-core socket")]
    parent_socket: String,

    #[arg(required = true, help = "Application command to execute (e.g. code, google-chrome, ptyxis)")]
    command: String,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true, help = "Arguments to pass to the application")]
    args: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();
    let pid = std::process::id();
    let island_socket = format!("wayland-island-{}", pid);

    info!("🚀 [KNOT LAUNCHER] Launching application '{}' inside Knot Island...", args.command);
    info!("🏝️ Generated isolated private socket: {}", island_socket);

    // 1. Spawn knot-island daemon
    let island_bin = std::env::current_exe()?
        .parent()
        .unwrap()
        .join("knot-island");

    let mut island_process = Command::new(&island_bin)
        .arg("--socket")
        .arg(&island_socket)
        .arg("--parent-socket")
        .arg(&args.parent_socket)
        .spawn()?;

    // 2. Poll for socket readiness
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    let socket_path = PathBuf::from(runtime_dir).join(&island_socket);

    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    if !ready {
        error!("❌ Knot Island socket failed to initialize within timeout");
        let _ = island_process.kill();
        std::process::exit(1);
    }

    info!("✅ Knot Island ready! Spawning '{}'...", args.command);

    // 3. Resolve direct native binary for VS Code / Chrome / Terminal to prevent wrapper fork & exit
    let mut exec_target = args.command.clone();
    let mut app_args = args.args.clone();

    if args.command == "code" || args.command.ends_with("/code") {
        if PathBuf::from("/usr/share/code/code").exists() {
            exec_target = "/usr/share/code/code".to_string();
        }
        if !app_args.iter().any(|a| a.contains("ozone-platform")) {
            app_args.push("--ozone-platform=wayland".to_string());
        }
        if !app_args.iter().any(|a| a.contains("disable-gpu")) {
            app_args.push("--disable-gpu".to_string());
        }
        if !app_args.iter().any(|a| a.contains("disable-gpu-sandbox")) {
            app_args.push("--disable-gpu-sandbox".to_string());
        }
        if !app_args.iter().any(|a| a.contains("user-data-dir")) {
            let tmp_dir = format!("/tmp/knot-vscode-{}", pid);
            let _ = std::fs::create_dir_all(&tmp_dir);
            app_args.push(format!("--user-data-dir={}", tmp_dir));
        }
    } else if args.command == "google-chrome" || args.command == "chromium" {
        if !app_args.iter().any(|a| a.contains("ozone-platform")) {
            app_args.push("--ozone-platform=wayland".to_string());
        }
        if !app_args.iter().any(|a| a.contains("disable-gpu")) {
            app_args.push("--disable-gpu".to_string());
        }
        if !app_args.iter().any(|a| a.contains("user-data-dir")) {
            let tmp_dir = format!("/tmp/knot-chrome-{}", pid);
            let _ = std::fs::create_dir_all(&tmp_dir);
            app_args.push(format!("--user-data-dir={}", tmp_dir));
        }
    } else if args.command == "ptyxis" {
        if !app_args.iter().any(|a| a == "-s" || a == "--standalone") {
            app_args.push("-s".to_string());
        }
    }

    // 4. Launch target application with Wayland environment variables
    let mut app_cmd = Command::new(&exec_target);
    app_cmd
        .args(&app_args)
        .env("WAYLAND_DISPLAY", &island_socket)
        .env("GDK_BACKEND", "wayland")
        .env("GSK_RENDERER", "gl")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("ELECTRON_OZONE_PLATFORM_HINT", "wayland")
        .env("CHROME_OZONE_PLATFORM_HINT", "wayland")
        .env("MOZ_ENABLE_WAYLAND", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = match app_cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            error!("❌ Failed to spawn application '{}': {}", exec_target, err);
            let _ = island_process.kill();
            return Err(err.into());
        }
    };

    // 5. Wait for app exit and cleanup island daemon
    let status = child.wait()?;
    info!("🏁 Application '{}' exited with status: {:?}. Cleaning up island daemon...", exec_target, status);
    let _ = island_process.kill();

    Ok(())
}
