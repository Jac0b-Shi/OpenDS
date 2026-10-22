//! 最小 HID 枚举与读写封装，手写 FFI，不引入外部 crate。
//!
//! 只覆盖 OpenDS 需要的部分：枚举 HID 接口、读属性与 caps、同步读报告。

use std::ffi::c_void;
use std::ptr;
use std::{io, sync::Mutex};

#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: *mut c_void,
}

struct IoEvent(*mut c_void);
impl Drop for IoEvent {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub const DIGCF_PRESENT: u32 = 0x02;
pub const DIGCF_DEVICEINTERFACE: u32 = 0x10;
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const FILE_SHARE_READ: u32 = 0x01;
pub const FILE_SHARE_WRITE: u32 = 0x02;
pub const OPEN_EXISTING: u32 = 3;
pub const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[repr(C)]
struct SpDeviceInterfaceData {
    cb_size: u32,
    interface_class_guid: Guid,
    flags: u32,
    reserved: usize,
}

#[repr(C)]
#[derive(Default)]
pub struct HiddAttributes {
    pub size: u32,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version_number: u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct HidpCaps {
    pub usage: u16,
    pub usage_page: u16,
    pub input_report_byte_length: u16,
    pub output_report_byte_length: u16,
    pub feature_report_byte_length: u16,
    pub reserved: [u16; 17],
    pub number_link_collection_nodes: u16,
    pub number_input_button_caps: u16,
    pub number_input_value_caps: u16,
    pub number_input_data_indices: u16,
    pub number_output_button_caps: u16,
    pub number_output_value_caps: u16,
    pub number_output_data_indices: u16,
    pub number_feature_button_caps: u16,
    pub number_feature_value_caps: u16,
    pub number_feature_data_indices: u16,
}

#[link(name = "setupapi")]
extern "system" {
    fn SetupDiGetClassDevsW(
        class_guid: *const Guid,
        enumerator: *const u16,
        hwnd_parent: *mut c_void,
        flags: u32,
    ) -> *mut c_void;
    fn SetupDiEnumDeviceInterfaces(
        dev_info: *mut c_void,
        dev_info_data: *mut c_void,
        interface_class_guid: *const Guid,
        member_index: u32,
        interface_data: *mut SpDeviceInterfaceData,
    ) -> i32;
    fn SetupDiGetDeviceInterfaceDetailW(
        dev_info: *mut c_void,
        interface_data: *mut SpDeviceInterfaceData,
        interface_detail: *mut c_void,
        detail_size: u32,
        required_size: *mut u32,
        device_info_data: *mut c_void,
    ) -> i32;
    fn SetupDiDestroyDeviceInfoList(dev_info: *mut c_void) -> i32;
}

#[link(name = "hid")]
extern "system" {
    fn HidD_GetHidGuid(guid: *mut Guid);
    fn HidD_GetAttributes(handle: *mut c_void, attrs: *mut HiddAttributes) -> i32;
    fn HidD_GetPreparsedData(handle: *mut c_void, preparsed: *mut *mut c_void) -> i32;
    fn HidP_GetCaps(preparsed: *mut c_void, caps: *mut HidpCaps) -> i32;
    fn HidD_FreePreparsedData(preparsed: *mut c_void) -> i32;
    fn HidD_GetSerialNumberString(handle: *mut c_void, buffer: *mut c_void, bytes: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        path: *const u16,
        desired_access: u32,
        share_mode: u32,
        security: *mut c_void,
        creation: u32,
        flags: u32,
        template: *mut c_void,
    ) -> *mut c_void;
    fn ReadFile(
        handle: *mut c_void,
        buffer: *mut c_void,
        to_read: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WriteFile(
        handle: *mut c_void,
        buffer: *const c_void,
        to_write: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn DeviceIoControl(
        handle: *mut c_void,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        bytes: *mut u32,
        op: *mut c_void,
    ) -> i32;
    fn CreateEventW(sa: *mut c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
    fn GetOverlappedResultEx(
        handle: *mut c_void,
        op: *mut Overlapped,
        bytes: *mut u32,
        timeout: u32,
        alertable: i32,
    ) -> i32;
    fn GetOverlappedResult(
        handle: *mut c_void,
        op: *mut Overlapped,
        bytes: *mut u32,
        wait: i32,
    ) -> i32;
    fn CancelIoEx(handle: *mut c_void, op: *mut Overlapped) -> i32;
}

/// 一个已打开的 HID 设备。
pub struct HidDevice {
    handle: *mut c_void,
    read_lock: Mutex<()>,
    write_lock: Mutex<()>,
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version_number: u16,
    pub input_len: u16,
    pub output_len: u16,
    pub feature_len: u16,
}

impl HidDevice {
    /// 枚举所有 HID 接口路径。
    pub fn enum_paths() -> Vec<String> {
        let mut guid = Guid::default();
        unsafe { HidD_GetHidGuid(&mut guid) };

        let dev_info = unsafe {
            SetupDiGetClassDevsW(
                &guid,
                ptr::null(),
                ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        };
        if dev_info == INVALID_HANDLE_VALUE || dev_info.is_null() {
            return Vec::new();
        }

        let mut paths = Vec::new();
        let mut index = 0u32;
        loop {
            let mut iface = SpDeviceInterfaceData {
                cb_size: std::mem::size_of::<SpDeviceInterfaceData>() as u32,
                interface_class_guid: Guid::default(),
                flags: 0,
                reserved: 0,
            };
            let ok = unsafe {
                SetupDiEnumDeviceInterfaces(dev_info, ptr::null_mut(), &guid, index, &mut iface)
            };
            if ok == 0 {
                break;
            }

            let mut required = 0u32;
            unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    dev_info,
                    &mut iface,
                    ptr::null_mut(),
                    0,
                    &mut required,
                    ptr::null_mut(),
                )
            };

            if required > 0 {
                // SP_DEVICE_INTERFACE_DETAIL_DATA_W：DWORD cbSize + WCHAR 路径
                let mut buf = vec![0u8; required as usize];
                let cb_size: u32 = if cfg!(target_pointer_width = "64") {
                    8
                } else {
                    6
                };
                buf[0..4].copy_from_slice(&cb_size.to_le_bytes());
                let ok = unsafe {
                    SetupDiGetDeviceInterfaceDetailW(
                        dev_info,
                        &mut iface,
                        buf.as_mut_ptr() as *mut c_void,
                        required,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    )
                };
                if ok != 0 {
                    let wide = unsafe {
                        std::slice::from_raw_parts(
                            buf[4..].as_ptr() as *const u16,
                            (buf.len() - 4) / 2,
                        )
                    };
                    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
                    paths.push(String::from_utf16_lossy(&wide[..end]));
                }
            }
            index += 1;
        }
        unsafe { SetupDiDestroyDeviceInfoList(dev_info) };
        paths
    }

    /// 打开指定路径的 HID 设备，读属性与报告长度。
    pub fn open(path: &str) -> Option<Self> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0x4000_0000, // FILE_FLAG_OVERLAPPED：读写必须可超时/取消。
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return None;
        }

        let mut attrs = HiddAttributes {
            size: std::mem::size_of::<HiddAttributes>() as u32,
            ..Default::default()
        };
        if unsafe { HidD_GetAttributes(handle, &mut attrs) } == 0 {
            unsafe { CloseHandle(handle) };
            return None;
        }

        let mut caps = HidpCaps::default();
        let mut preparsed: *mut c_void = ptr::null_mut();
        if unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } != 0 {
            unsafe { HidP_GetCaps(preparsed, &mut caps) };
            unsafe { HidD_FreePreparsedData(preparsed) };
        }

        Some(Self {
            handle,
            read_lock: Mutex::new(()),
            write_lock: Mutex::new(()),
            path: path.to_string(),
            vendor_id: attrs.vendor_id,
            product_id: attrs.product_id,
            version_number: attrs.version_number,
            input_len: caps.input_report_byte_length,
            output_len: caps.output_report_byte_length,
            feature_len: caps.feature_report_byte_length,
        })
    }

    /// 等待有界的一帧；超时与设备断开分别返回 None 和 Err。
    pub fn read_result(&self, timeout_ms: u32) -> io::Result<Option<Vec<u8>>> {
        let _guard = self
            .read_lock
            .lock()
            .map_err(|_| io::Error::other("HID 读锁损坏"))?;
        let size = self.input_len.max(64) as usize;
        let mut buf = vec![0u8; size];
        let mut read = 0u32;
        let event = self.event()?;
        let mut op = Overlapped {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: event.0,
        };
        let ok = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                size as u32,
                &mut read,
                &mut op as *mut Overlapped as *mut c_void,
            )
        };
        if !self.complete(ok, &mut op, &mut read, timeout_ms)? {
            return Ok(None);
        }
        if read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "HID 已断开"));
        }
        buf.truncate(read as usize);
        Ok(Some(buf))
    }

    pub fn read(&self, timeout_ms: usize) -> Option<Vec<u8>> {
        self.read_result(timeout_ms.min(u32::MAX as usize) as u32)
            .ok()
            .flatten()
    }

    /// 写入一份输出报告。
    pub fn write(&self, data: &[u8]) -> bool {
        self.write_result(data, 100).is_ok()
    }

    pub fn write_result(&self, data: &[u8], timeout_ms: u32) -> io::Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| io::Error::other("HID 写锁损坏"))?;
        if data.is_empty() || data.len() > self.output_len as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HID 输出长度无效",
            ));
        }
        let mut padded = vec![0u8; self.output_len as usize];
        padded[..data.len()].copy_from_slice(data);
        let mut written = 0u32;
        let event = self.event()?;
        let mut op = Overlapped {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: event.0,
        };
        let ok = unsafe {
            WriteFile(
                self.handle,
                padded.as_ptr() as *const c_void,
                padded.len() as u32,
                &mut written,
                &mut op as *mut Overlapped as *mut c_void,
            )
        };
        if !self.complete(ok, &mut op, &mut written, timeout_ms)? {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "HID 写入超时"));
        }
        if written as usize != padded.len() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "HID 写入不完整"));
        }
        Ok(())
    }

    fn event(&self) -> io::Result<IoEvent> {
        let h = unsafe { CreateEventW(ptr::null_mut(), 1, 0, ptr::null()) };
        if h.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(IoEvent(h))
        }
    }

    fn complete(
        &self,
        started: i32,
        op: &mut Overlapped,
        bytes: &mut u32,
        timeout: u32,
    ) -> io::Result<bool> {
        if started != 0 {
            return Ok(true);
        }
        let code = unsafe { GetLastError() };
        if code != 997 {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        if unsafe { GetOverlappedResultEx(self.handle, op, bytes, timeout.max(1), 0) } != 0 {
            return Ok(true);
        }
        let code = unsafe { GetLastError() };
        // 无论取消是否与正常完成竞争，都等到驱动不再使用 op/缓冲后才能返回。
        unsafe {
            CancelIoEx(self.handle, op);
            GetOverlappedResult(self.handle, op, bytes, 1);
        }
        if code == 258 || code == 996 {
            Ok(false)
        } else {
            Err(io::Error::from_raw_os_error(code as i32))
        }
    }

    pub fn serial_number(&self) -> Option<String> {
        let mut b = [0u16; 128];
        if unsafe { HidD_GetSerialNumberString(self.handle, b.as_mut_ptr() as *mut c_void, 256) }
            == 0
        {
            return None;
        }
        let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        let s = String::from_utf16_lossy(&b[..n]);
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    pub fn is_dualsense_bluetooth(&self) -> bool {
        self.is_sony() && matches!(self.product_id, 0x0CE6 | 0x0DF2) && self.input_len == 78
    }

    /// 固件特性报告 0x20；仅查询，不修改手柄配置。
    pub fn dualsense_firmware(&self) -> Option<u16> {
        let report = self.feature_report(0x20, 64, 500).ok()?;
        (report[0] == 0x20).then(|| u16::from_le_bytes([report[44], report[45]]))
    }

    pub fn feature_report(&self, id: u8, length: usize, timeout: u32) -> io::Result<Vec<u8>> {
        if !(2..=1024).contains(&length) {
            return Err(io::Error::other("特性报告长度无效"));
        }
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| io::Error::other("HID 写锁损坏"))?;
        let mut report = vec![0u8; length.max(self.feature_len as usize)];
        report[0] = id;
        let event = self.event()?;
        let mut op = Overlapped {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: event.0,
        };
        let mut bytes = 0;
        // hidclass.h: HID_OUT_CTL_CODE(100)，首字节为请求的 report ID。
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                0x000B0192,
                ptr::null(),
                0,
                report.as_mut_ptr().cast(),
                report.len() as u32,
                &mut bytes,
                (&mut op as *mut Overlapped).cast(),
            )
        };
        if !self.complete(ok, &mut op, &mut bytes, timeout)? {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "特性报告读取超时"));
        }
        report.truncate(length);
        if report[0] != id || report[1..].iter().all(|&b| b == 0) {
            return Err(io::Error::other("特性报告内容无效"));
        }
        Ok(report)
    }

    pub fn is_sony(&self) -> bool {
        self.vendor_id == 0x054C
    }
}

impl Drop for HidDevice {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

// 安全论证：`handle` 是内核 HID 对象句柄，Windows 允许同一句柄在不同线程上
// 并发进行读/写（方向不同，互不干扰）。这里用 `Arc<HidDevice>` 让主线程读输入、
// 音频线程写输出；唯一需要保证的是句柄只在最后一份引用释放时关闭，由 `Drop` 保证。
unsafe impl Send for HidDevice {}
unsafe impl Sync for HidDevice {}

#[cfg(test)]
mod tests {
    use super::*;
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateNamedPipeW(
            name: *const u16,
            open: u32,
            mode: u32,
            instances: u32,
            out_size: u32,
            in_size: u32,
            timeout: u32,
            sa: *mut c_void,
        ) -> *mut c_void;
    }
    #[test]
    fn native_read_timeout_cancels_before_reusing_buffer_and_preserves_bidirectional_io() {
        let name: Vec<u16> = format!("\\\\.\\pipe\\opends-io-test-{}", std::process::id())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let server =
            unsafe { CreateNamedPipeW(name.as_ptr(), 3, 0, 1, 4096, 4096, 0, ptr::null_mut()) };
        assert_ne!(server, INVALID_HANDLE_VALUE);
        let client = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null_mut(),
                OPEN_EXISTING,
                0x4000_0000,
                ptr::null_mut(),
            )
        };
        assert_ne!(client, INVALID_HANDLE_VALUE);
        let device = HidDevice {
            handle: client,
            read_lock: Mutex::new(()),
            write_lock: Mutex::new(()),
            path: String::new(),
            vendor_id: 0,
            product_id: 0,
            version_number: 0,
            input_len: 64,
            output_len: 64,
            feature_len: 0,
        };
        let server = server as usize;
        let peer = std::thread::spawn(move || {
            let server = IoEvent(server as *mut c_void);
            std::thread::sleep(std::time::Duration::from_millis(150));
            let mut n = 0;
            let data = [1u8, 2, 3, 4];
            assert_ne!(
                unsafe { WriteFile(server.0, data.as_ptr() as _, 4, &mut n, ptr::null_mut()) },
                0
            );
            let mut out = [0u8; 64];
            assert_ne!(
                unsafe { ReadFile(server.0, out.as_mut_ptr() as _, 64, &mut n, ptr::null_mut()) },
                0
            );
            out
        });
        let started = std::time::Instant::now();
        let timed = device.read_result(20).unwrap();
        let elapsed = started.elapsed();
        let received = device.read_result(300).unwrap();
        device.write_result(&[0x31, 0x10], 300).unwrap();
        let sent = peer.join().unwrap();
        assert!(timed.is_none(), "超时读取不能等待到对端的数据");
        assert!(elapsed < std::time::Duration::from_millis(120));
        assert_eq!(received.unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(&sent[..2], &[0x31, 0x10]);
        assert!(sent[2..].iter().all(|&x| x == 0));
    }
}
