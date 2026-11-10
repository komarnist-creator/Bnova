//! Системные функции Буста (только Windows): память, программы, службы, питание, ярлык.
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const NO_WINDOW: u32 = 0x0800_0000; // CREATE_NO_WINDOW
pub const SERVICES: [&str; 3] = ["SysMain", "DiagTrack", "WSearch"];
pub const HIGH_GUID: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const BALANCED_GUID: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

// ---------------------------------------------------------------- пути и настройки

pub fn app_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Bnova")
}

pub fn app_exe() -> PathBuf {
    app_dir().join("bnova.exe")
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub do_close: bool,
    pub do_ram: bool,
    pub do_services: bool,
    pub do_power: bool,
    pub do_temp: bool,
    pub force_kill: bool,
    pub auto_clean: bool,
    pub auto_mb: u32,
    pub excluded: Vec<String>,
    pub shortcut_ver: u32,
    pub prev_scheme: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            do_close: true,
            do_ram: true,
            do_services: true,
            do_power: true,
            do_temp: true,
            force_kill: false,
            auto_clean: false,
            auto_mb: 400,
            excluded: Vec::new(),
            shortcut_ver: 0,
            prev_scheme: String::new(),
        }
    }
}

fn cfg_path() -> PathBuf {
    app_dir().join("bnova.json")
}

pub fn load_cfg() -> Config {
    std::fs::read_to_string(cfg_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_cfg(c: &Config) {
    let _ = std::fs::create_dir_all(app_dir());
    if let Ok(s) = serde_json::to_string_pretty(c) {
        let _ = std::fs::write(cfg_path(), s);
    }
}

// ---------------------------------------------------------------- нативный модуль (boost.c)

#[repr(C)]
#[derive(Clone, Copy)]
struct RawApp {
    pid: u32,
    ws_mb: u32,
    name: [u16; 64],
    title: [u16; 128],
}

extern "C" {
    fn boost_ram_mb(total: *mut u64, avail: *mut u64) -> i32;
    fn boost_trim_all() -> i32;
    fn boost_purge_standby() -> i32;
    fn boost_list_apps(out: *mut RawApp, max: i32) -> i32;
    fn boost_close_pid(pid: u32) -> i32;
    fn boost_is_alive(pid: u32) -> i32;
    fn boost_kill_pid(pid: u32) -> i32;
    fn boost_is_admin() -> i32;
}

#[derive(Clone)]
pub struct AppItem {
    pub pid: u32,
    pub name: String,  // имя exe, например chrome.exe
    pub title: String, // заголовок окна
    pub ws_mb: u32,
    pub selected: bool,
}

impl AppItem {
    pub fn pretty_name(&self) -> String {
        let n = self.name.as_str();
        if n.len() > 4 && n.is_char_boundary(n.len() - 4) && n[n.len() - 4..].eq_ignore_ascii_case(".exe") {
            n[..n.len() - 4].to_string()
        } else {
            n.to_string()
        }
    }

    pub fn label(&self) -> String {
        let mut t: String = self.title.chars().take(34).collect();
        if self.title.chars().count() > 34 {
            t.push('…');
        }
        format!("{}  -  {}  ({} МБ)", self.pretty_name(), t, self.ws_mb)
    }
}

fn from_wide(s: &[u16]) -> String {
    let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..n])
}

/// (всего МБ, доступно МБ)
pub fn ram_info() -> Option<(u64, u64)> {
    let (mut total, mut avail) = (0u64, 0u64);
    let ok = unsafe { boost_ram_mb(&mut total, &mut avail) };
    if ok == 0 {
        return None;
    }
    Some((total, avail))
}

pub fn trim_all() -> i32 {
    unsafe { boost_trim_all() }
}

pub fn purge_standby() -> i32 {
    unsafe { boost_purge_standby() }
}

pub fn list_apps() -> Vec<AppItem> {
    let mut buf = vec![RawApp { pid: 0, ws_mb: 0, name: [0u16; 64], title: [0u16; 128] }; 64];
    let n = unsafe { boost_list_apps(buf.as_mut_ptr(), buf.len() as i32) };
    let n = (n.max(0) as usize).min(buf.len());
    buf.iter()
        .take(n)
        .map(|r| AppItem {
            pid: r.pid,
            name: from_wide(&r.name),
            title: from_wide(&r.title),
            ws_mb: r.ws_mb,
            selected: true,
        })
        .collect()
}

pub fn close_pid(pid: u32) {
    unsafe {
        boost_close_pid(pid);
    }
}

pub fn is_alive(pid: u32) -> bool {
    unsafe { boost_is_alive(pid) != 0 }
}

pub fn kill_pid(pid: u32) -> bool {
    unsafe { boost_kill_pid(pid) != 0 }
}

pub fn is_admin() -> bool {
    unsafe { boost_is_admin() != 0 }
}

// ---------------------------------------------------------------- команды, службы, питание

/// Запускает программу без окна. Возвращает (успех, вывод).
pub fn run_hidden(prog: &str, args: &[&str]) -> (bool, String) {
    match Command::new(prog)
        .args(args)
        .creation_flags(NO_WINDOW)
        .stdin(Stdio::null())
        .output()
    {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string()),
        Err(_) => (false, String::new()),
    }
}

pub fn stop_services() -> usize {
    let mut n = 0;
    for s in SERVICES {
        if run_hidden("net", &["stop", s, "/y"]).0 {
            n += 1;
        }
    }
    n
}

pub fn start_services() -> usize {
    let mut n = 0;
    for s in SERVICES {
        if run_hidden("net", &["start", s]).0 {
            n += 1;
        }
    }
    n
}

fn find_guid(s: &str) -> Option<String> {
    s.split(|c: char| c.is_whitespace() || c == ':')
        .find(|p| p.len() == 36 && p.matches('-').count() == 4)
        .map(|p| p.to_string())
}

pub fn current_scheme() -> Option<String> {
    let (ok, out) = run_hidden("powercfg", &["/getactivescheme"]);
    if ok {
        find_guid(&out)
    } else {
        None
    }
}

pub fn set_high_performance() -> bool {
    if run_hidden("powercfg", &["/setactive", HIGH_GUID]).0 {
        return true;
    }
    let (ok, out) = run_hidden("powercfg", &["/duplicatescheme", HIGH_GUID]);
    if ok {
        if let Some(g) = find_guid(&out) {
            return run_hidden("powercfg", &["/setactive", g.as_str()]).0;
        }
    }
    false
}

pub fn restore_scheme(prev: &str) -> bool {
    let g = if prev.len() == 36 { prev } else { BALANCED_GUID };
    run_hidden("powercfg", &["/setactive", g]).0
}

// ---------------------------------------------------------------- временные файлы

fn clean_dir(dir: &Path, min_age: Duration) -> u64 {
    let mut freed = 0u64;
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    for e in rd.flatten() {
        let p = e.path();
        let md = match e.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let old_enough = md
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| d >= min_age)
            .unwrap_or(true);
        if !old_enough {
            continue;
        }
        if md.is_dir() {
            freed += clean_dir(&p, min_age);
            let _ = std::fs::remove_dir(&p);
        } else {
            let size = md.len();
            if std::fs::remove_file(&p).is_ok() {
                freed += size;
            }
        }
    }
    freed
}

/// Удаляет старые (старше часа) временные файлы. Возвращает освобождённые байты.
pub fn clean_temp() -> u64 {
    let age = Duration::from_secs(3600);
    let mut total = clean_dir(&std::env::temp_dir(), age);
    if let Some(win) = std::env::var_os("WINDIR") {
        total += clean_dir(&PathBuf::from(win).join("Temp"), age);
    }
    total
}

// ---------------------------------------------------------------- ярлык, окно выбора файла

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Копирует exe в постоянную папку, чтобы скачанный файл можно было удалить.
pub fn install_self() -> bool {
    let run = || -> std::io::Result<bool> {
        let cur = std::env::current_exe()?;
        let dst = app_exe();
        std::fs::create_dir_all(app_dir())?;
        if same_path(&cur, &dst) {
            return Ok(false);
        }
        std::fs::copy(&cur, &dst)?;
        Ok(true)
    };
    run().unwrap_or(false)
}

/// Ярлык «Bnova» на рабочем столе.
pub fn create_shortcut() -> bool {
    let target = if app_exe().exists() {
        app_exe()
    } else {
        match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return false,
        }
    };
    let t = target.to_string_lossy().replace('\'', "''");
    let wd = target
        .parent()
        .map(|p| p.to_string_lossy().replace('\'', "''"))
        .unwrap_or_default();
    let ps = format!(
        "$d=[Environment]::GetFolderPath('Desktop');Remove-Item -LiteralPath ($d+'\\Буст.lnk') -ErrorAction SilentlyContinue;$s=(New-Object -ComObject WScript.Shell).CreateShortcut($d+'\\Bnova.lnk');$s.TargetPath='{t}';$s.WorkingDirectory='{wd}';$s.IconLocation='{t},0';$s.Description='Bnova - Буст от nova';$s.Save()"
    );
    run_hidden("powershell", &["-NoProfile", "-Command", ps.as_str()]).0
}

/// Перезапуск от имени администратора (на случай, если Windows не запросила права сама).
pub fn relaunch_admin() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let p = exe.to_string_lossy().replace('\'', "''");
    let ps = format!("Start-Process -FilePath '{p}' -Verb RunAs");
    run_hidden("powershell", &["-NoProfile", "-Command", ps.as_str()]).0
}
