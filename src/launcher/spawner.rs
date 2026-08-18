use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use tracing::{error, info};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppEntry {
    pub id: usize,
    pub title: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
}

pub struct AppCatalog;

impl AppCatalog {
    pub fn default_catalog() -> Vec<AppEntry> {
        let mut apps = Vec::new();
        let mut id = 1;

        // 1. VS Code
        if PathBuf::from("/usr/bin/code").exists() || PathBuf::from("/usr/share/code/code").exists() {
            apps.push(AppEntry {
                id,
                title: "Visual Studio Code".to_string(),
                description: "Code Editor & Collaborative IDE".to_string(),
                command: "code".to_string(),
                args: vec![".".to_string()],
            });
            id += 1;
        }

        // 2. Google Chrome / Chromium
        if PathBuf::from("/usr/bin/google-chrome").exists() || PathBuf::from("/usr/bin/google-chrome-stable").exists() {
            apps.push(AppEntry {
                id,
                title: "Google Chrome".to_string(),
                description: "Fast & Secure Web Browser".to_string(),
                command: "google-chrome".to_string(),
                args: vec![],
            });
            id += 1;
        } else if PathBuf::from("/usr/bin/chromium").exists() || PathBuf::from("/usr/bin/chromium-browser").exists() {
            apps.push(AppEntry {
                id,
                title: "Chromium Browser".to_string(),
                description: "Open-source Web Browser".to_string(),
                command: "chromium".to_string(),
                args: vec![],
            });
            id += 1;
        }

        // 3. Ptyxis / GNOME Terminal
        if PathBuf::from("/usr/bin/ptyxis").exists() {
            apps.push(AppEntry {
                id,
                title: "Ptyxis Terminal".to_string(),
                description: "Fast Modern Terminal & Shell".to_string(),
                command: "ptyxis".to_string(),
                args: vec!["-s".to_string()],
            });
            id += 1;
        } else if PathBuf::from("/usr/bin/gnome-terminal").exists() {
            apps.push(AppEntry {
                id,
                title: "GNOME Terminal".to_string(),
                description: "Command Line Shell".to_string(),
                command: "gnome-terminal".to_string(),
                args: vec!["--standalone".to_string()],
            });
            id += 1;
        }

        // 4. GNOME Text Editor
        if PathBuf::from("/usr/bin/gnome-text-editor").exists() {
            apps.push(AppEntry {
                id,
                title: "GNOME Text Editor".to_string(),
                description: "Clean Document & Note Editor".to_string(),
                command: "gnome-text-editor".to_string(),
                args: vec!["--standalone".to_string(), "--new-window".to_string()],
            });
            id += 1;
        } else if PathBuf::from("/usr/bin/gedit").exists() {
            apps.push(AppEntry {
                id,
                title: "Gedit Text Editor".to_string(),
                description: "General Purpose Text Editor".to_string(),
                command: "gedit".to_string(),
                args: vec!["--new-window".to_string()],
            });
            id += 1;
        }

        // 5. Files / Nautilus
        if PathBuf::from("/usr/bin/nautilus").exists() {
            apps.push(AppEntry {
                id,
                title: "Files & Documents".to_string(),
                description: "File Manager & Explorer".to_string(),
                command: "nautilus".to_string(),
                args: vec!["--new-window".to_string()],
            });
            id += 1;
        }

        // 6. Calculator
        if PathBuf::from("/usr/bin/gnome-calculator").exists() {
            apps.push(AppEntry {
                id,
                title: "Calculator".to_string(),
                description: "Math, Scientific & Unit Tool".to_string(),
                command: "gnome-calculator".to_string(),
                args: vec![],
            });
            id += 1;
        }

        // 7. System Monitor
        if PathBuf::from("/usr/bin/gnome-system-monitor").exists() {
            apps.push(AppEntry {
                id,
                title: "System Monitor".to_string(),
                description: "CPU, Memory & Process Inspector".to_string(),
                command: "gnome-system-monitor".to_string(),
                args: vec![],
            });
        }

        apps
    }

    pub fn spawn_island_app(parent_socket: &str, app: &AppEntry) -> Result<(), String> {
        let parent_sock = parent_socket.to_string();
        let app_clone = app.clone();

        info!("🚀 [IN-COMPOSITOR SPAWNER] Spawning '{}' ({:?}) into isolated Knot Island...", app.title, app.command);

        // Spawn in a detached background thread so compositor frame loop is never stalled
        thread::spawn(move || {
            let pid = std::process::id();
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let island_socket = format!("wayland-island-{}-{}", pid, timestamp % 100000);

            // 1. Locate knot-island binary
            let island_bin = match std::env::current_exe() {
                Ok(exe) => {
                    let dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));
                    let path = dir.join("knot-island");
                    if path.exists() {
                        path
                    } else {
                        PathBuf::from("./target/debug/knot-island")
                    }
                }
                Err(_) => PathBuf::from("./target/debug/knot-island"),
            };

            let mut island_process = match Command::new(&island_bin)
                .arg("--socket")
                .arg(&island_socket)
                .arg("--parent-socket")
                .arg(&parent_sock)
                .spawn() {
                    Ok(child) => child,
                    Err(err) => {
                        error!("❌ Failed to spawn knot-island daemon for {}: {:?}", app_clone.title, err);
                        return;
                    }
                };

            // 2. Poll for socket availability
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
            let socket_path = PathBuf::from(runtime_dir).join(&island_socket);

            let mut ready = false;
            for _ in 0..60 {
                if socket_path.exists() {
                    ready = true;
                    break;
                }
                thread::sleep(Duration::from_millis(40));
            }

            if !ready {
                error!("❌ Timeout waiting for island socket: {}", island_socket);
                let _ = island_process.kill();
                return;
            }

            // 3. Resolve binary & isolation arguments
            let mut exec_target = app_clone.command.clone();
            let mut app_args = app_clone.args.clone();

            if app_clone.command == "code" || app_clone.command.ends_with("/code") {
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
                    let tmp_dir = format!("/tmp/knot-vscode-{}-{}", pid, timestamp % 10000);
                    let _ = std::fs::create_dir_all(&tmp_dir);
                    app_args.push(format!("--user-data-dir={}", tmp_dir));
                }
            } else if app_clone.command == "google-chrome" || app_clone.command == "chromium" {
                if !app_args.iter().any(|a| a.contains("ozone-platform")) {
                    app_args.push("--ozone-platform=wayland".to_string());
                }
                if !app_args.iter().any(|a| a.contains("disable-gpu")) {
                    app_args.push("--disable-gpu".to_string());
                }
                if !app_args.iter().any(|a| a.contains("user-data-dir")) {
                    let tmp_dir = format!("/tmp/knot-chrome-{}-{}", pid, timestamp % 10000);
                    let _ = std::fs::create_dir_all(&tmp_dir);
                    app_args.push(format!("--user-data-dir={}", tmp_dir));
                }
            }

            // 4. Launch guest app inside private island
            info!("🎉 Spawning guest binary '{}' on {}...", exec_target, island_socket);
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
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            match app_cmd.spawn() {
                Ok(mut child) => {
                    let _ = child.wait();
                    let _ = island_process.kill();
                }
                Err(err) => {
                    error!("❌ Failed to spawn guest app '{}': {:?}", exec_target, err);
                    let _ = island_process.kill();
                }
            }
        });

        Ok(())
    }
}
