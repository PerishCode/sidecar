use std::net::SocketAddr;

#[cfg(target_os = "linux")]
pub fn listeners(pid: u32) -> Result<Vec<SocketAddr>, String> {
    linux::listeners(pid)
}

#[cfg(target_os = "macos")]
pub fn listeners(pid: u32) -> Result<Vec<SocketAddr>, String> {
    macos::listeners(pid)
}

#[cfg(windows)]
pub fn listeners(pid: u32) -> Result<Vec<SocketAddr>, String> {
    windows::listeners(pid)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn listeners(_pid: u32) -> Result<Vec<SocketAddr>, String> {
    Err("TCP listener discovery is not implemented on this platform".to_string())
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn dedup(mut addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    addrs.sort();
    addrs.dedup();
    addrs
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{dedup, table};
    use std::collections::BTreeSet;
    use std::fs;
    use std::net::SocketAddr;

    pub fn listeners(pid: u32) -> Result<Vec<SocketAddr>, String> {
        let inodes = inodes(pid)?;
        if inodes.is_empty() {
            return Ok(Vec::new());
        }
        let mut addrs = Vec::new();
        for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
            let text =
                fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}"))?;
            addrs.extend(table(&text, &inodes)?);
        }
        Ok(dedup(addrs))
    }

    fn inodes(pid: u32) -> Result<BTreeSet<String>, String> {
        let dir = format!("/proc/{pid}/fd");
        let entries = fs::read_dir(&dir).map_err(|err| format!("failed to read {dir}: {err}"))?;
        let mut inodes = BTreeSet::new();
        for entry in entries {
            let entry = entry.map_err(|err| format!("failed to read {dir} entry: {err}"))?;
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            let text = target.to_string_lossy();
            let Some(inode) = text
                .strip_prefix("socket:[")
                .and_then(|value| value.strip_suffix(']'))
            else {
                continue;
            };
            inodes.insert(inode.to_string());
        }
        Ok(inodes)
    }
}

#[cfg(target_os = "linux")]
#[doc(hidden)]
pub fn table(
    text: &str,
    inodes: &std::collections::BTreeSet<String>,
) -> Result<Vec<SocketAddr>, String> {
    let mut addrs = Vec::new();
    for line in text.lines().skip(1) {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() <= 9 {
            continue;
        }
        if columns[3] != "0A" || !inodes.contains(columns[9]) {
            continue;
        }
        addrs.push(address(columns[1])?);
    }
    Ok(addrs)
}

#[cfg(target_os = "linux")]
fn address(value: &str) -> Result<SocketAddr, String> {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    let (addr, port) = value
        .split_once(':')
        .ok_or_else(|| format!("invalid /proc tcp local_address {value:?}"))?;
    let port = u16::from_str_radix(port, 16)
        .map_err(|err| format!("invalid /proc tcp port {port:?}: {err}"))?;
    let ip = match addr.len() {
        8 => {
            let raw = u32::from_str_radix(addr, 16)
                .map_err(|err| format!("invalid /proc tcp IPv4 address {addr:?}: {err}"))?;
            IpAddr::V4(Ipv4Addr::from(raw.to_le_bytes()))
        }
        32 => {
            let mut bytes = [0u8; 16];
            for index in 0..4 {
                let start = index * 8;
                let word = u32::from_str_radix(&addr[start..start + 8], 16)
                    .map_err(|err| format!("invalid /proc tcp IPv6 address {addr:?}: {err}"))?;
                bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            IpAddr::V6(Ipv6Addr::from(bytes))
        }
        _ => return Err(format!("invalid /proc tcp address width {addr:?}")),
    };
    Ok(SocketAddr::new(ip, port))
}

#[cfg(target_os = "macos")]
mod macos {
    use super::dedup;
    use std::net::SocketAddr;
    use std::process::Command;

    pub fn listeners(pid: u32) -> Result<Vec<SocketAddr>, String> {
        let output = Command::new("lsof")
            .args([
                "-Pan",
                "-p",
                &pid.to_string(),
                "-iTCP",
                "-sTCP:LISTEN",
                "-n",
            ])
            .output()
            .map_err(|err| format!("lsof failed: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "lsof exited with status: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(dedup(super::lsof::listeners(&stdout)))
    }
}

pub mod lsof {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[doc(hidden)]
    pub fn listeners(text: &str) -> Vec<SocketAddr> {
        text.lines()
            .filter(|line| line.contains("(LISTEN)"))
            .filter_map(|line| line.split_whitespace().find_map(endpoint))
            .collect()
    }

    fn endpoint(value: &str) -> Option<SocketAddr> {
        let endpoint = value.strip_prefix("TCP").unwrap_or(value);
        let endpoint = endpoint.trim_start_matches('*').trim_start_matches('@');
        if let Ok(addr) = endpoint.parse::<SocketAddr>() {
            return Some(addr);
        }
        let port = endpoint.rsplit_once(':')?.1.parse::<u16>().ok()?;
        Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
    }
}

#[cfg(windows)]
mod windows {
    use super::{dedup, port};
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::slice;
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

    pub fn listeners(pid: u32) -> Result<Vec<SocketAddr>, String> {
        let mut addrs = ipv4(pid)?;
        addrs.extend(ipv6(pid)?);
        Ok(dedup(addrs))
    }

    fn ipv4(pid: u32) -> Result<Vec<SocketAddr>, String> {
        let table = query(AF_INET as u32)?;
        if table.len() < size_of::<MIB_TCPTABLE_OWNER_PID>() {
            return Ok(Vec::new());
        }
        let header = table.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
        let count = unsafe { (*header).dwNumEntries as usize };
        let first = unsafe { (*header).table.as_ptr() };
        let rows = unsafe { slice::from_raw_parts(first, count) };
        Ok(rows
            .iter()
            .filter(|row| row.dwOwningPid == pid)
            .map(|row| {
                SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                    port(row.dwLocalPort),
                )
            })
            .collect())
    }

    fn ipv6(pid: u32) -> Result<Vec<SocketAddr>, String> {
        let table = query(AF_INET6 as u32)?;
        if table.len() < size_of::<MIB_TCP6TABLE_OWNER_PID>() {
            return Ok(Vec::new());
        }
        let header = table.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID;
        let count = unsafe { (*header).dwNumEntries as usize };
        let first = unsafe { (*header).table.as_ptr() };
        let rows = unsafe { slice::from_raw_parts(first, count) };
        Ok(rows
            .iter()
            .filter(|row| row.dwOwningPid == pid)
            .map(|row| {
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                    port(row.dwLocalPort),
                )
            })
            .collect())
    }

    fn query(family: u32) -> Result<Vec<u8>, String> {
        let mut size = 0u32;
        let first = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if first != ERROR_INSUFFICIENT_BUFFER && first != NO_ERROR {
            return Err(format!("GetExtendedTcpTable size query failed: {first}"));
        }
        let mut table = vec![0u8; size as usize];
        let status = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr() as *mut c_void,
                &mut size,
                0,
                family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if status != NO_ERROR {
            return Err(format!("GetExtendedTcpTable failed: {status}"));
        }
        Ok(table)
    }
}

#[doc(hidden)]
pub fn port(value: u32) -> u16 {
    u16::from_be((value & 0xffff) as u16)
}
