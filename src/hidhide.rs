//! HidHide 持久黑名单绑定，带备份与崩溃恢复。
//!
//! 背景：本机驱动（1.4.181.0）**不支持** session blacklist（IOCTL 2056/2057 返回
//! `ERROR_INVALID_PARAMETER`），只能用持久黑名单。持久配置会写注册表，一旦程序
//! 异常退出，用户的手柄会一直对所有游戏不可见。因此这里做了完整的安全网：
//!
//! 1. 改动前先读原状态并**落盘备份**
//! 2. 正常退出时恢复并删除备份
//! 3. 下次启动若发现残留备份，说明上次异常退出，自动恢复
//!
//! 设备实例 ID 使用 `SetupDiGetDeviceInstanceId` 格式，例如
//! `HID\VID_054C&PID_0CE6&MI_03\8&96661A0&0&0000`。

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;

const DEVICE_PATH: &str = r"\\.\HidHide";
const BACKUP_FILE: &str = "hidhide-state.txt";

/// `CTL_CODE(0x8001, N, METHOD_BUFFERED, FILE_READ_DATA)`
const fn ctl(n: u32) -> u32 {
    (0x8001u32 << 16) | (1u32 << 14) | (n << 2)
}

const IOCTL_GET_BLACKLIST: u32 = ctl(2050);
const IOCTL_SET_BLACKLIST: u32 = ctl(2051);
const IOCTL_GET_ACTIVE: u32 = ctl(2052);
const IOCTL_SET_ACTIVE: u32 = ctl(2053);
const IOCTL_GET_WHITELIST: u32 = ctl(2048);
const IOCTL_SET_WHITELIST: u32 = ctl(2049);

const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_ALL: u32 = 0x07;
const OPEN_EXISTING: u32 = 3;
const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        path: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        creation: u32,
        flags: u32,
        template: *mut c_void,
    ) -> *mut c_void;
    fn DeviceIoControl(
        device: *mut c_void,
        code: u32,
        in_buf: *const c_void,
        in_size: u32,
        out_buf: *mut c_void,
        out_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn QueryDosDeviceW(name: *const u16, target: *mut u16, size: u32) -> u32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// HidHide 控制设备句柄。
pub struct HidHide {
    handle: *mut c_void,
}

impl HidHide {
    pub fn open() -> Option<Self> {
        let path = wide(DEVICE_PATH);
        let h = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_ALL,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_null() {
            None
        } else {
            Some(Self { handle: h })
        }
    }

    /// 读取持久黑名单。空列表返回 Some(vec![])，驱动不可用返回 None。
    pub fn get_blacklist(&self) -> Option<Vec<String>> {
        // 先问所需大小：传 0 通常返回 ERROR_MORE_DATA 并给出 needed
        let mut needed = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_GET_BLACKLIST,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
                &mut needed,
                ptr::null_mut(),
            )
        };
        // 给足余量，避免两次调用的边界问题
        let size = (needed as usize).max(8192);
        let mut buf = vec![0u8; size];
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_GET_BLACKLIST,
                ptr::null(),
                0,
                buf.as_mut_ptr() as *mut c_void,
                size as u32,
                &mut ret,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return None;
        }
        Some(parse_multi_sz(&buf[..ret as usize]))
    }

    /// 写入持久黑名单（整体替换）。
    pub fn set_blacklist(&self, ids: &[String]) -> bool {
        let payload = to_multi_sz(ids);
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_SET_BLACKLIST,
                payload.as_ptr() as *const c_void,
                payload.len() as u32,
                ptr::null_mut(),
                0,
                &mut ret,
                ptr::null_mut(),
            )
        };
        ok != 0
    }

    pub fn get_active(&self) -> Option<bool> {
        let mut out = [0u8; 1];
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_GET_ACTIVE,
                ptr::null(),
                0,
                out.as_mut_ptr() as *mut c_void,
                1,
                &mut ret,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            None
        } else {
            Some(out[0] != 0)
        }
    }

    pub fn set_active(&self, on: bool) -> bool {
        let input = [on as u8];
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_SET_ACTIVE,
                input.as_ptr() as *const c_void,
                1,
                ptr::null_mut(),
                0,
                &mut ret,
                ptr::null_mut(),
            )
        };
        ok != 0
    }

    /// 读取白名单：被允许"看穿"隐藏程序的应用程序完整路径。
    ///
    /// 格式为 `\Device\HarddiskVolume3\...\app.exe`，比盘符形式更稳。
    pub fn get_whitelist(&self) -> Option<Vec<String>> {
        let mut needed = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_GET_WHITELIST,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
                &mut needed,
                ptr::null_mut(),
            )
        };
        let size = (needed as usize).max(8192);
        let mut buf = vec![0u8; size];
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_GET_WHITELIST,
                ptr::null(),
                0,
                buf.as_mut_ptr() as *mut c_void,
                size as u32,
                &mut ret,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return None;
        }
        Some(parse_multi_sz(&buf[..ret as usize]))
    }

    /// 写入白名单（整体替换）。
    pub fn set_whitelist(&self, paths: &[String]) -> bool {
        let payload = to_multi_sz(paths);
        let mut ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_SET_WHITELIST,
                payload.as_ptr() as *const c_void,
                payload.len() as u32,
                ptr::null_mut(),
                0,
                &mut ret,
                ptr::null_mut(),
            )
        };
        ok != 0
    }
}

impl Drop for HidHide {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

/// 改动前的原始状态。
#[derive(Debug, Clone, Default)]
pub struct Backup {
    pub active: bool,
    pub blacklist: Vec<String>,
    pub added: Vec<String>,
    pub whitelist_added: Vec<String>,
    pub version: u8,
}

/// 备份文件路径：`%LOCALAPPDATA%\OpenDS\hidhide-state.txt`
pub fn backup_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let mut p = PathBuf::from(base);
    p.push("OpenDS");
    Some(p.join(BACKUP_FILE))
}

fn serialize_backup(b: &Backup) -> String {
    let mut s = format!("version={}\nactive={}\n", b.version, u8::from(b.active));
    for id in &b.blacklist {
        s.push_str(&format!("dev={id}\n"));
    }
    for id in &b.added {
        s.push_str(&format!("added={id}\n"));
    }
    for p in &b.whitelist_added {
        s.push_str(&format!("app={p}\n"));
    }
    s
}

/// 读取残留备份（存在说明上次异常退出）。
fn parse_backup(text: &str) -> Result<Backup, String> {
    let mut b = Backup {
        version: 1,
        ..Default::default()
    };
    let mut active_seen = false;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("active=") {
            if active_seen {
                return Err(
                    crate::tr!(hidhide_duplicate_active_key_in_hidhide_recovery_journal).into(),
                );
            }
            active_seen = true;
            b.active = match v {
                "1" => true,
                "0" => false,
                _ => return Err(crate::tr!(hidhide_empty_hidhide_recovery_journal).into()),
            };
        } else if let Some(v) = line.strip_prefix("version=") {
            b.version = v
                .parse()
                .map_err(|_| crate::tr!(hidhide_invalid_hidhide_recovery_journal_version))?;
        } else if let Some(v) = line.strip_prefix("dev=") {
            if !v.is_empty() {
                b.blacklist.push(v.to_owned());
            }
        } else if let Some(v) = line.strip_prefix("added=") {
            if !v.is_empty() {
                b.added.push(v.to_owned());
            }
        } else if let Some(v) = line.strip_prefix("app=") {
            if !v.is_empty() {
                b.whitelist_added.push(v.to_owned());
            }
        }
    }
    if !active_seen || !matches!(b.version, 1 | 2) {
        return Err(
            crate::tr!(hidhide_corrupt_or_unsupported_hidhide_recovery_journal_file).into(),
        );
    }
    Ok(b)
}

pub fn load_backup() -> Result<Option<Backup>, String> {
    let p = backup_path().ok_or(crate::tr!(
        hidhide_localappdata_is_not_set_cannot_locate_the
    ))?;
    match std::fs::read_to_string(p) {
        Ok(s) => parse_backup(&s).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(crate::tr!(
            hidhide_could_not_read_hidhide_recovery_journal,
            error = crate::i18n_arg!(e)
        )),
    }
}

fn contains(items: &[String], s: &str) -> bool {
    items.iter().any(|x| x.eq_ignore_ascii_case(s))
}
fn same(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().all(|s| contains(b, s))
}
trait HideDriver {
    fn black(&self) -> Option<Vec<String>>;
    fn white(&self) -> Option<Vec<String>>;
    fn active(&self) -> Option<bool>;
    fn write_black(&self, v: &[String]) -> bool;
    fn write_white(&self, v: &[String]) -> bool;
    fn write_active(&self, v: bool) -> bool;
}
impl HideDriver for HidHide {
    fn black(&self) -> Option<Vec<String>> {
        self.get_blacklist()
    }
    fn white(&self) -> Option<Vec<String>> {
        self.get_whitelist()
    }
    fn active(&self) -> Option<bool> {
        self.get_active()
    }
    fn write_black(&self, v: &[String]) -> bool {
        self.set_blacklist(v)
    }
    fn write_white(&self, v: &[String]) -> bool {
        self.set_whitelist(v)
    }
    fn write_active(&self, v: bool) -> bool {
        self.set_active(v)
    }
}
fn restore_state(h: &impl HideDriver, b: &Backup) -> Result<(), String> {
    let current = h.black().ok_or(crate::tr!(
        hidhide_could_not_read_hidhide_device_blacklist_recovery
    ))?;
    let white = h.white().ok_or(crate::tr!(
        hidhide_could_not_read_hidhide_application_whitelist_recovery
    ))?;
    let active = h.active().ok_or(crate::tr!(
        hidhide_could_not_read_hidhide_state_recovery_journal
    ))?;
    // v2 只撤销 OpenDS 自己新增的条目，保留用户运行期间的增删。
    let target: Vec<String> = if b.version == 1 {
        b.blacklist.clone()
    } else {
        current
            .into_iter()
            .filter(|s| !contains(&b.added, s))
            .collect()
    };
    let target_white: Vec<String> = white
        .into_iter()
        .filter(|s| !contains(&b.whitelist_added, s))
        .collect();
    let foreign = target.iter().any(|s| !contains(&b.blacklist, s));
    let target_active = if b.version == 1 {
        b.active
    } else if !b.active && !foreign {
        false
    } else {
        active
    };
    if !h.write_black(&target) || !h.write_white(&target_white) || !h.write_active(target_active) {
        return Err(crate::tr!(hidhide_could_not_restore_hidhide_state_recovery_journal).into());
    }
    if !h.black().is_some_and(|v| same(&v, &target))
        || !h.white().is_some_and(|v| same(&v, &target_white))
        || h.active() != Some(target_active)
    {
        return Err(crate::tr!(
            hidhide_hidhide_recovery_verification_failed_recovery_journal_retained
        )
        .into());
    }
    Ok(())
}

/// 上次异常退出留下的恢复动作。
///
/// 返回被恢复的备份内容；没有残留返回 None。
pub fn recover_if_needed() -> Result<Option<Backup>, String> {
    let Some(b) = load_backup()? else {
        return Ok(None);
    };
    let hh = HidHide::open().ok_or(crate::tr!(
        hidhide_hidhide_unavailable_cannot_restore_yet_recovery_journal
    ))?;
    restore_journal(
        &hh,
        &b,
        &backup_path().ok_or(crate::tr!(hidhide_could_not_read_the_recovery_path))?,
    )?;
    Ok(Some(b))
}

fn restore_journal(h: &impl HideDriver, b: &Backup, path: &std::path::Path) -> Result<(), String> {
    restore_state(h, b)?;
    std::fs::remove_file(path).map_err(|e| {
        crate::tr!(
            hidhide_hidhide_restored_but_its_recovery_journal_could,
            error = crate::i18n_arg!(e)
        )
    })
}

/// 隐藏期间的状态守卫：构造时保存并应用，Drop 时恢复。
pub struct Hider {
    hh: HidHide,
    backup: Backup,
    restored: bool,
}

impl Hider {
    /// 把 `device_ids` 追加进黑名单，并确保 device hiding 处于开启状态。
    ///
    /// 驱动不可用或读取原状态失败时返回错误，不修改驱动。
    pub fn engage(device_ids: &[String]) -> Result<Self, String> {
        let path =
            backup_path().ok_or(crate::tr!(hidhide_localappdata_is_not_set_refusing_to_hide))?;
        if path.exists() {
            return Err(
                crate::tr!(hidhide_unfinished_hidhide_recovery_found_run_restore_first).into(),
            );
        }
        let hh = HidHide::open().ok_or(crate::tr!(hidhide_hidhide_driver_unavailable))?;
        let active = hh
            .get_active()
            .ok_or(crate::tr!(hidhide_could_not_read_hidhide_state))?;
        let blacklist = hh
            .get_blacklist()
            .ok_or(crate::tr!(hidhide_could_not_read_hidhide_device_blacklist))?;
        let mut whitelist = hh.get_whitelist().ok_or(crate::tr!(
            hidhide_could_not_read_hidhide_application_whitelist
        ))?;
        let exe = application_path(
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .display()
                .to_string(),
        )?;
        let added: Vec<String> = device_ids
            .iter()
            .filter(|id| !contains(&blacklist, id))
            .cloned()
            .collect();
        let whitelist_added = if contains(&whitelist, &exe) {
            vec![]
        } else {
            vec![exe.clone()]
        };
        let backup = Backup {
            active,
            blacklist: blacklist.clone(),
            added: added.clone(),
            whitelist_added,
            version: 2,
        };
        crate::persistence::atomic_write(&path, serialize_backup(&backup).as_bytes()).map_err(
            |e| {
                crate::tr!(
                    hidhide_could_not_save_recovery_journal_driver_unchanged,
                    error = crate::i18n_arg!(e)
                )
            },
        )?;
        let mut guard = Self {
            hh,
            backup,
            restored: false,
        };
        let mut new_list = blacklist;
        new_list.extend(added);
        if !contains(&whitelist, &exe) {
            whitelist.push(exe);
        }
        if !guard.hh.set_whitelist(&whitelist)
            || !guard.hh.set_blacklist(&new_list)
            || !guard.hh.set_active(true)
            || !guard
                .hh
                .get_blacklist()
                .is_some_and(|v| same(&v, &new_list))
            || !guard
                .hh
                .get_whitelist()
                .is_some_and(|v| same(&v, &whitelist))
            || guard.hh.get_active() != Some(true)
        {
            let rollback = guard.restore();
            return Err(crate::tr!(
                hidhide_could_not_hide_the_physical_controller,
                result = crate::i18n_arg!(rollback
                    .err()
                    .unwrap_or_else(|| crate::tr!(hidhide_rolled_back).into()))
            ));
        }
        Ok(guard)
    }

    pub fn restore(&mut self) -> Result<(), String> {
        if self.restored {
            return Ok(());
        }
        restore_journal(
            &self.hh,
            &self.backup,
            &backup_path().ok_or(crate::tr!(hidhide_could_not_read_the_recovery_path))?,
        )?;
        self.restored = true;
        Ok(())
    }

    /// 当前生效的黑名单（调试用）。
    pub fn current_blacklist(&self) -> Option<Vec<String>> {
        self.hh.get_blacklist()
    }

    /// Reuse the session handle: the driver can deny a second control open.
    pub fn is_active(&self) -> bool {
        !self.restored && self.hh.get_active() == Some(true)
    }
}

impl Drop for Hider {
    fn drop(&mut self) {
        if let Err(e) = self.restore() {
            eprintln!("{e}");
        }
    }
}

/// 把 HID 接口路径转换成设备实例 ID。
///
/// `\\?\hid#vid_054c&pid_0ce6&mi_03#8&96661a0&0&0000#{guid}`
/// → `HID\VID_054C&PID_0CE6&MI_03\8&96661A0&0&0000`
pub fn interface_path_to_instance_id(path: &str) -> Option<String> {
    let body = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\\.\"))?;
    let body = match body.rfind('#') {
        Some(i) => &body[..i],
        None => body,
    };
    let mut parts = body.split('#');
    let head = parts.next()?;
    let dev = parts.next()?;
    let inst = parts.next()?;
    if parts.next().is_some() || !head.eq_ignore_ascii_case("hid") {
        return None;
    }
    Some(format!(
        "HID\\{}\\{}",
        dev.to_ascii_uppercase(),
        inst.to_ascii_uppercase()
    ))
}

/// HidHide 存储 NT 卷路径；不直接写 C:\ 或不受支持的 UNC 路径。
pub fn application_path(path: &str) -> Result<String, String> {
    if path.contains(['\r', '\n', '\0']) {
        return Err(crate::tr!(hidhide_invalid_characters_in_application_path).into());
    }
    if path.starts_with(r"\Device\") {
        return Ok(path.to_owned());
    }
    let full = std::fs::canonicalize(path).map_err(|e| {
        crate::tr!(
            hidhide_application_path_unavailable,
            error = crate::i18n_arg!(e)
        )
    })?;
    let s = full.display().to_string();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    if s.as_bytes().get(1) != Some(&b':') {
        return Err(crate::tr!(hidhide_select_an_application_on_a_local_disk).into());
    }
    let drive = wide(&s[..2]);
    let mut target = [0u16; 32768];
    if unsafe { QueryDosDeviceW(drive.as_ptr(), target.as_mut_ptr(), target.len() as u32) } == 0 {
        return Err(crate::tr!(
            hidhide_could_not_convert_application_path,
            error = crate::i18n_arg!(std::io::Error::last_os_error())
        ));
    }
    let n = target
        .iter()
        .position(|&c| c == 0)
        .ok_or(crate::tr!(hidhide_invalid_application_path))?;
    Ok(format!(
        "{}{}",
        String::from_utf16_lossy(&target[..n]),
        &s[2..]
    ))
}

pub fn add_application(path: &str) -> Result<(), String> {
    let p = application_path(path)?;
    let h = HidHide::open().ok_or(crate::tr!(hidhide_hidhide_driver_unavailable))?;
    let mut v = h
        .get_whitelist()
        .ok_or(crate::tr!(hidhide_could_not_read_application_whitelist))?;
    if !contains(&v, &p) {
        v.push(p);
    }
    if !h.set_whitelist(&v) || !h.get_whitelist().is_some_and(|r| same(&r, &v)) {
        return Err(crate::tr!(hidhide_could_not_write_or_verify_application_whitelist).into());
    }
    Ok(())
}
pub fn remove_application(path: &str) -> Result<(), String> {
    let h = HidHide::open().ok_or(crate::tr!(hidhide_hidhide_driver_unavailable))?;
    let v: Vec<String> = h
        .get_whitelist()
        .ok_or(crate::tr!(hidhide_could_not_read_application_whitelist))?
        .into_iter()
        .filter(|p| !p.eq_ignore_ascii_case(path))
        .collect();
    if !h.set_whitelist(&v) || !h.get_whitelist().is_some_and(|r| same(&r, &v)) {
        return Err(crate::tr!(hidhide_could_not_write_or_verify_application_whitelist).into());
    }
    Ok(())
}

fn to_multi_sz(items: &[String]) -> Vec<u8> {
    let mut w: Vec<u16> = Vec::new();
    for s in items {
        w.extend(s.encode_utf16());
        w.push(0);
    }
    // MULTI_SZ 必须以空串（双 null）结尾。列表非空时最后一个 push 已经补了一个 0，
    // 列表为空时必须补两个，否则总长只有 2 字节，驱动会判为非法参数。
    if items.is_empty() {
        w.push(0);
    }
    w.push(0);
    let mut bytes = Vec::with_capacity(w.len() * 2);
    for c in w {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    bytes
}

fn parse_multi_sz(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if bytes.len() < 4 {
        return out;
    }
    let chars: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    for part in chars.split(|&c| c == 0) {
        if part.is_empty() {
            break;
        }
        out.push(String::from_utf16_lossy(part));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_codes_match_driver_contract() {
        assert_eq!(IOCTL_GET_BLACKLIST, 0x8001_6008);
        assert_eq!(IOCTL_SET_BLACKLIST, 0x8001_600C);
        assert_eq!(IOCTL_GET_ACTIVE, 0x8001_6010);
        assert_eq!(IOCTL_SET_ACTIVE, 0x8001_6014);
    }

    #[test]
    fn convert_usb_hid_path() {
        let p = r"\\?\hid#vid_054c&pid_0ce6&mi_03#8&96661a0&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
        assert_eq!(
            interface_path_to_instance_id(p).unwrap(),
            r"HID\VID_054C&PID_0CE6&MI_03\8&96661A0&0&0000"
        );
    }

    #[test]
    fn multi_sz_roundtrip() {
        let items = vec![
            r"HID\VID_054C&PID_0CE6&MI_03\8&96661A0&0&0000".to_string(),
            r"HID\VID_054C&PID_0CE6&MI_03\B&1A2B0D7F&0&0000".to_string(),
        ];
        let bytes = to_multi_sz(&items);
        assert_eq!(bytes.len() % 2, 0);
        assert_eq!(parse_multi_sz(&bytes), items);
    }

    #[test]
    fn empty_list_roundtrip() {
        let bytes = to_multi_sz(&[]);
        assert_eq!(bytes.len(), 4);
        assert!(parse_multi_sz(&bytes).is_empty());
    }

    #[test]
    fn backup_serialization_roundtrip() {
        // 直接测序列化格式，不落盘
        let b = Backup {
            active: true,
            blacklist: vec!["A".into(), "B".into()],
            ..Default::default()
        };
        let mut s = String::new();
        s.push_str(if b.active { "active=1\n" } else { "active=0\n" });
        for id in &b.blacklist {
            s.push_str(&format!("dev={id}\n"));
        }
        let mut parsed = Backup::default();
        for line in s.lines() {
            if let Some(v) = line.strip_prefix("active=") {
                parsed.active = v.trim() == "1";
            } else if let Some(v) = line.strip_prefix("dev=") {
                parsed.blacklist.push(v.to_string());
            }
        }
        assert_eq!(parsed.active, b.active);
        assert_eq!(parsed.blacklist, b.blacklist);
    }

    #[test]
    fn journal_rejects_corruption_and_roundtrips_owned_changes() {
        assert!(parse_backup("").is_err());
        assert!(parse_backup("active=perhaps").is_err());
        let b = Backup {
            active: false,
            blacklist: vec!["user".into()],
            added: vec!["ours".into()],
            whitelist_added: vec!["app".into()],
            version: 2,
        };
        let v = parse_backup(&serialize_backup(&b)).unwrap();
        assert_eq!(v.added, b.added);
        assert_eq!(v.whitelist_added, b.whitelist_added);
    }
    #[derive(Default)]
    struct Fake {
        black: std::cell::RefCell<Vec<String>>,
        white: std::cell::RefCell<Vec<String>>,
        active: std::cell::Cell<bool>,
        fail: bool,
    }
    impl HideDriver for Fake {
        fn black(&self) -> Option<Vec<String>> {
            Some(self.black.borrow().clone())
        }
        fn white(&self) -> Option<Vec<String>> {
            Some(self.white.borrow().clone())
        }
        fn active(&self) -> Option<bool> {
            Some(self.active.get())
        }
        fn write_black(&self, v: &[String]) -> bool {
            if self.fail {
                return false;
            }
            *self.black.borrow_mut() = v.to_vec();
            true
        }
        fn write_white(&self, v: &[String]) -> bool {
            *self.white.borrow_mut() = v.to_vec();
            true
        }
        fn write_active(&self, v: bool) -> bool {
            self.active.set(v);
            true
        }
    }
    #[test]
    fn recovery_preserves_concurrent_user_changes() {
        let f = Fake::default();
        *f.black.borrow_mut() = vec!["ours".into(), "new-user-device".into()];
        *f.white.borrow_mut() = vec!["our-app".into(), "user-app".into()];
        f.active.set(true);
        let b = Backup {
            version: 2,
            active: false,
            blacklist: vec!["removed-by-user".into()],
            added: vec!["ours".into()],
            whitelist_added: vec!["our-app".into()],
        };
        restore_state(&f, &b).unwrap();
        assert_eq!(*f.black.borrow(), vec!["new-user-device"]);
        assert_eq!(*f.white.borrow(), vec!["user-app"]);
        assert!(f.active.get());
    }
    #[test]
    fn failed_driver_restore_keeps_actual_journal_for_retry() {
        let path =
            std::env::temp_dir().join(format!("opends-journal-test-{}.txt", std::process::id()));
        let backup = Backup {
            version: 2,
            added: vec!["ours".into()],
            ..Default::default()
        };
        let bytes = serialize_backup(&backup);
        std::fs::write(&path, &bytes).unwrap();
        let failing = Fake {
            fail: true,
            ..Default::default()
        };
        assert!(restore_journal(&failing, &backup, &path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        restore_journal(&Fake::default(), &backup, &path).unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn recovery_restores_owned_switch_and_reports_driver_failure() {
        let f = Fake::default();
        f.active.set(true);
        *f.black.borrow_mut() = vec!["ours".into()];
        let b = Backup {
            version: 2,
            added: vec!["ours".into()],
            ..Default::default()
        };
        restore_state(&f, &b).unwrap();
        assert!(!f.active.get());
        assert!(restore_state(
            &Fake {
                fail: true,
                ..Default::default()
            },
            &b
        )
        .is_err());
    }
    #[test]
    fn multi_sz_preserves_non_bmp_program_names() {
        let v = vec!["C:\\程序\\🎮.exe".to_string()];
        assert_eq!(parse_multi_sz(&to_multi_sz(&v)), v);
    }
}
