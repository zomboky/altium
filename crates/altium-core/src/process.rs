//! Finds the `claude` process an Altium process runs under. Hooks and the
//! MCP server share it, which ties MCP changes to the session the
//! `SessionStart` hook recorded.

/// Process names of Claude Code: the native build, or Node for npm installs.
const CLAUDE_NAMES: [&str; 2] = ["claude", "node"];

/// How many ancestors are inspected.
const MAX_DEPTH: usize = 12;

/// PID of the closest ancestor of this process that is Claude Code.
pub fn claude_ancestor() -> Option<u32> {
    let chain = ancestors(std::process::id());
    // Prefer an ancestor named `claude`; fall back to Node for npm installs.
    for wanted in CLAUDE_NAMES {
        if let Some((pid, _)) = chain.iter().find(|(_, name)| name == wanted) {
            return Some(*pid);
        }
    }
    None
}

/// Lowercase executable names without `.exe`.
fn normalize(name: &str) -> String {
    let name = name.to_lowercase();
    name.strip_suffix(".exe").unwrap_or(&name).to_owned()
}

/// Ancestors of `pid`, closest first, with their name.
#[cfg(target_os = "linux")]
fn ancestors(pid: u32) -> Vec<(u32, String)> {
    let mut chain = Vec::new();
    let mut current = pid;
    for _ in 0..MAX_DEPTH {
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{current}/stat")) else {
            break;
        };
        // `pid (comm) state ppid …`; comm may contain spaces and parentheses.
        let Some(close) = stat.rfind(')') else { break };
        let Some(ppid) = stat[close + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|p| p.parse::<u32>().ok())
        else {
            break;
        };
        if ppid <= 1 {
            break;
        }
        let name = std::fs::read_to_string(format!("/proc/{ppid}/comm")).unwrap_or_default();
        let mut name = normalize(name.trim());
        // Node shows as `node` in comm but as `claude` in its command line.
        if name == "node"
            && std::fs::read(format!("/proc/{ppid}/cmdline"))
                .is_ok_and(|cmd| String::from_utf8_lossy(&cmd).contains("claude"))
        {
            name = "claude".into();
        }
        chain.push((ppid, name));
        current = ppid;
    }
    chain
}

#[cfg(windows)]
fn ancestors(pid: u32) -> Vec<(u32, String)> {
    use std::collections::HashMap;

    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    fn created(pid: u32) -> Option<u64> {
        // SAFETY: plain Win32 calls on a handle we own and close.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let zero = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let (mut creation, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
            let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
            CloseHandle(handle);
            (ok != 0).then(|| (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
        }
    }

    let mut parents: HashMap<u32, (u32, String)> = HashMap::new();
    // SAFETY: the snapshot handle is checked and closed; the entry is sized.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            parents.insert(entry.th32ProcessID, (entry.th32ParentProcessID, normalize(&name)));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }

    let mut chain = Vec::new();
    let mut current = pid;
    for _ in 0..MAX_DEPTH {
        let Some((parent, _)) = parents.get(&current) else {
            break;
        };
        let Some((_, name)) = parents.get(parent) else { break };
        // Windows reuses PIDs: a real parent started before its child.
        match (created(*parent), created(current)) {
            (Some(p), Some(c)) if p <= c => {}
            _ => break,
        }
        chain.push((*parent, name.clone()));
        current = *parent;
    }
    chain
}

#[cfg(not(any(target_os = "linux", windows)))]
fn ancestors(_pid: u32) -> Vec<(u32, String)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_up_from_the_test_process() {
        // The test runner always has a parent (cargo), whatever runs it.
        let chain = ancestors(std::process::id());
        assert!(!chain.is_empty());
        assert!(chain.iter().all(|(pid, _)| *pid != std::process::id()));
    }

    #[test]
    fn names_are_normalized() {
        assert_eq!(normalize("Claude.EXE"), "claude");
        assert_eq!(normalize("node"), "node");
    }
}
