//! Per-OS locations and tool discovery.

use std::env;
use std::path::{Path, PathBuf};

static DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Use `dir` for all data in this process (`tx mcp --data-dir`). Agents start MCP servers with a
/// cleaned environment, so `tx run` passes its own data folder explicitly.
pub fn set_data_dir(dir: PathBuf) {
    let _ = DATA_DIR.set(dir);
}

/// Persistent data (database, Whisper models): ~/.local/share/tx, ~/Library/Application Support/tx,
/// %LOCALAPPDATA%\tx, `$TX_DATA_DIR`, or the folder given to `set_data_dir`.
pub fn data_dir() -> PathBuf {
    let p = DATA_DIR.get().cloned().unwrap_or_else(|| {
        env::var_os("TX_DATA_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::data_local_dir()
                    .unwrap_or_else(env::temp_dir)
                    .join("tx")
            })
    });
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Disposable transcript cache.
pub fn cache_dir() -> PathBuf {
    let p = dirs::cache_dir()
        .unwrap_or_else(env::temp_dir)
        .join("tx")
        .join("transcripts");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// PATH plus the usual per-user install dirs. Apps started from Finder, Start or a desktop menu
/// get a minimal PATH that misses e.g. ~/.local/bin, where Claude Code installs itself.
pub fn search_path() -> std::ffi::OsString {
    let mut parts: Vec<PathBuf> = env::var_os("PATH")
        .map(|p| env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        for rel in [
            ".local/bin",
            ".claude/local",
            ".bun/bin",
            ".npm-global/bin",
            ".volta/bin",
        ] {
            parts.push(home.join(rel));
        }
    }
    if cfg!(windows) {
        if let Some(appdata) = env::var_os("APPDATA") {
            parts.push(PathBuf::from(appdata).join("npm"));
        }
    } else {
        parts.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    }
    env::join_paths(parts).unwrap_or_default()
}

/// Find `name` on the extended search path (adds `.exe`/`.cmd` on Windows).
pub fn which(name: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    env::split_paths(&search_path()).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

/// A tool tx manages: its own copy in the data dir (downloaded and kept current by `ytdlp`),
/// else the user's own install.
pub fn bundled(name: &str) -> Option<PathBuf> {
    let own = data_dir()
        .join("bin")
        .join(format!("{name}{}", env::consts::EXE_SUFFIX));
    Some(own).filter(|p| p.is_file()).or_else(|| which(name))
}

/// A child process with the extended PATH and, on Windows, no console window flash.
pub fn command(program: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.env("PATH", search_path()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// Agents start MCP servers with a minimal environment. On Linux that drops the desktop session
/// bus, so yt-dlp can't unlock the browser's cookie key in the keyring (signed-in features like
/// `import_subscriptions` see a logged-out YouTube) and notifications fail. Restore the session
/// variables from the systemd user manager.
///
/// Sets environment variables, so call it before any other thread starts.
#[cfg(target_os = "linux")]
pub fn restore_session_env() {
    use std::os::unix::fs::MetadataExt;
    const VARS: [&str; 7] = [
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
        "DISPLAY",
        "WAYLAND_DISPLAY",
    ];
    if env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return;
    }
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let uid = std::fs::metadata("/proc/self").ok()?.uid();
            Some(PathBuf::from(format!("/run/user/{uid}")))
        });
    let Some(runtime) = runtime.filter(|d| d.is_dir()) else {
        return;
    };
    let Ok(out) = std::process::Command::new("systemctl")
        .args(["--user", "show-environment"])
        .env("XDG_RUNTIME_DIR", &runtime)
        .output()
    else {
        return;
    };
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((key, value)) = line.split_once('=')
            && VARS.contains(&key)
            && env::var_os(key).is_none()
        {
            // SAFETY: called at the start of main, before any other thread exists.
            unsafe { env::set_var(key, value) };
        }
    }
}
