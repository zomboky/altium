//! `altium serve`, `altium open` and `altium service`: the local dashboard,
//! opening it, and starting it with the user session.

use std::env;
use std::process::{Command, Stdio};

use altium_core::config::{self, GlobalConfig};
use altium_core::registry::Registry;
use altium_core::store::{Store, find_root};
use altium_server::{AppState, ServeError};
use anyhow::{Context, Result, bail};

use crate::display;
use crate::http;

pub fn port(explicit: Option<u16>) -> Result<u16> {
    if let Some(port) = explicit {
        return Ok(port);
    }
    let registry = Registry::for_user()?;
    Ok(GlobalConfig::load(registry.dir())?.port())
}

pub fn is_running(port: u16) -> bool {
    matches!(http::get("127.0.0.1", port, "/api/health"), Ok((200, body)) if body.contains("\"ok\":true"))
}

pub fn run(port: Option<u16>, background: bool) -> Result<()> {
    if background {
        detach_console();
    }
    let registry = Registry::for_user()?;
    let config_file = registry.dir().join(config::FILE_NAME);
    let config = GlobalConfig::load(registry.dir())?;
    let port = port.unwrap_or(config.port());
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let listener = altium_server::bind(port).await?;
        if !background {
            println!("Dashboard Altium : http://127.0.0.1:{port}");
            println!("Ctrl+C pour l'arrêter.");
        }
        let state = AppState::new(registry, port);
        altium_server::serve(listener, state, true)
            .await
            .map_err(ServeError::Io)
    });
    match result {
        Ok(()) => Ok(()),
        Err(ServeError::PortInUse(port)) => {
            let message = format!(
                "le port {port} est déjà utilisé{}. Choisis-en un autre avec `altium serve --port <PORT>`, \
                 ou durablement avec `port = <PORT>` dans {}",
                if is_running(port) {
                    " par un autre dashboard Altium"
                } else {
                    " par un autre programme"
                },
                display::path(&config_file)
            );
            if background {
                altium_core::log::write("serve", &message);
            }
            bail!(message)
        }
        Err(e) => {
            if background {
                altium_core::log::write("serve", &e.to_string());
            }
            Err(e.into())
        }
    }
}

/// `altium serve --background` runs without a console: on Windows, the
/// console window that started it closes right away.
fn detach_console() {
    #[cfg(windows)]
    // SAFETY: FreeConsole has no preconditions.
    unsafe {
        windows_sys::Win32::System::Console::FreeConsole();
    }
}

/// Starts `altium serve --background` as a detached process.
pub fn start_background(port: u16) -> Result<()> {
    let exe = env::current_exe()?;
    let mut command = Command::new(exe);
    command
        .args(["serve", "--background", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group: closing the terminal does not stop it.
        command.process_group(0);
    }
    command.spawn().context("lancement du dashboard en arrière-plan")?;
    for _ in 0..50 {
        if is_running(port) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!("le dashboard ne répond pas ; lance `altium serve` pour voir l'erreur")
}

pub fn open(port: Option<u16>) -> Result<()> {
    let port = self::port(port)?;
    let base = format!("http://127.0.0.1:{port}");
    let url = match find_root(&env::current_dir()?) {
        Some(root) => {
            let project = Store::open(&root)?.project()?;
            format!("{base}/projects/{}", project.id)
        }
        None => base,
    };
    if !is_running(port) {
        println!("Démarrage du dashboard en arrière-plan…");
        start_background(port)?;
    }
    println!("Ouverture de {url}");
    opener::open_browser(&url).context("impossible d'ouvrir le navigateur")?;
    Ok(())
}

pub fn install() -> Result<()> {
    let exe = dunce::canonicalize(env::current_exe()?)?;
    let port = port(None)?;
    platform::install(&exe)?;
    if !is_running(port) {
        start_background(port)?;
    }
    println!("Le dashboard démarrera avec ta session : http://127.0.0.1:{port}");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let port = port(None)?;
    platform::uninstall()?;
    if is_running(port) {
        let _ = http::post("127.0.0.1", port, "/api/shutdown");
    }
    println!("Le dashboard ne démarrera plus avec ta session.");
    Ok(())
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::PathBuf;

    use super::*;

    fn unit_path() -> Result<PathBuf> {
        Ok(dirs::config_dir()
            .context("dossier de configuration introuvable")?
            .join("systemd/user/altium.service"))
    }

    fn systemctl(args: &[&str]) -> Result<()> {
        let status = Command::new("systemctl").arg("--user").args(args).status()?;
        if !status.success() {
            bail!("`systemctl --user {}` a échoué", args.join(" "));
        }
        Ok(())
    }

    pub fn install(exe: &std::path::Path) -> Result<()> {
        let path = unit_path()?;
        std::fs::create_dir_all(path.parent().expect("unit dir"))?;
        let unit = format!(
            "[Unit]\nDescription=Altium dashboard\nAfter=graphical-session.target\n\n\
             [Service]\nExecStart=\"{}\" serve\nRestart=on-failure\n\n\
             [Install]\nWantedBy=default.target\n",
            exe.display()
        );
        std::fs::write(&path, unit)?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", "--now", "altium.service"])?;
        println!("Service systemd utilisateur : {}", display::path(&path));
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let path = unit_path()?;
        let _ = systemctl(&["disable", "--now", "altium.service"]);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let _ = systemctl(&["daemon-reload"]);
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use super::*;

    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const AUMID_KEY: &str = r"HKCU\Software\Classes\AppUserModelId\Altium";

    fn reg(args: &[&str]) -> Result<bool> {
        let mut command = Command::new("reg");
        command.args(args).stdout(Stdio::null()).stderr(Stdio::null());
        altium_core::git::no_window(&mut command);
        Ok(command.status().context("reg.exe introuvable")?.success())
    }

    pub fn install(exe: &std::path::Path) -> Result<()> {
        let value = format!("\"{}\" serve --background", exe.display());
        if !reg(&["add", RUN_KEY, "/v", "Altium", "/t", "REG_SZ", "/d", &value, "/f"])? {
            bail!("impossible d'écrire la clé Run du compte");
        }
        // Notifications show under this name instead of « Windows PowerShell ».
        reg(&[
            "add",
            AUMID_KEY,
            "/v",
            "DisplayName",
            "/t",
            "REG_SZ",
            "/d",
            "Altium",
            "/f",
        ])?;
        println!("Démarrage automatique : valeur « Altium » de {RUN_KEY}");
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        reg(&["delete", RUN_KEY, "/v", "Altium", "/f"])?;
        reg(&["delete", AUMID_KEY, "/f"])?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use super::*;

    pub fn install(_exe: &std::path::Path) -> Result<()> {
        bail!("le service n'est pris en charge que sous Linux et Windows")
    }

    pub fn uninstall() -> Result<()> {
        bail!("le service n'est pris en charge que sous Linux et Windows")
    }
}
