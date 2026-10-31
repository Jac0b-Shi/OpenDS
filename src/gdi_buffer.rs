//! UI-thread scratch surfaces; pixels are redrawn on every use, never cached.
use std::{cell::RefCell, collections::HashMap};
use winapi::{
    shared::windef::*,
    um::{wingdi::*, winuser::*},
};
const LIMIT: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;
struct Buffer {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    width: i32,
    height: i32,
    bits: i32,
    planes: i32,
    bytes: usize,
}
impl Buffer {
    unsafe fn new(output: HDC, width: i32, height: i32, bits: i32, planes: i32) -> Option<Self> {
        let bytes = ((width as usize)
            .checked_mul(bits as usize)?
            .checked_mul(planes as usize)?
            .checked_add(31)?
            / 32)
            .checked_mul(4)?
            .checked_mul(height as usize)?;
        let dc = CreateCompatibleDC(output);
        if dc.is_null() {
            return None;
        }
        let bitmap = CreateCompatibleBitmap(output, width, height);
        if bitmap.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bitmap as _);
        if old.is_null() || old as isize == -1 {
            DeleteObject(bitmap as _);
            DeleteDC(dc);
            return None;
        }
        #[cfg(test)]
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        Some(Self {
            dc,
            bitmap,
            old,
            width,
            height,
            bits,
            planes,
            bytes,
        })
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap as _);
            DeleteDC(self.dc);
        }
    }
}
#[derive(Default)]
struct Cache {
    entries: HashMap<(usize, u8), (Buffer, u64)>,
    bytes: usize,
    tick: u64,
}
thread_local! {static CACHE:RefCell<Cache>=RefCell::new(Cache::default());}
#[cfg(test)]
thread_local! {static ALLOCATIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
/// A lease removes its surface from the map before drawing, so reentrant native
/// callbacks cannot borrow the same DC or hold the cache locked across messages.
///
/// `slot` separates draw regions that belong to one window but are rarely the
/// same size — the client area and the nonclient scrollbar, for instance — so
/// switching between them does not keep replacing a single cached bitmap.
pub struct PaintBuffer {
    buffer: Option<Buffer>,
    key: (usize, u8),
    saved: i32,
}
impl PaintBuffer {
    pub unsafe fn acquire(
        hwnd: HWND,
        slot: u8,
        output: HDC,
        width: i32,
        height: i32,
    ) -> Option<Self> {
        if output.is_null() || width <= 0 || height <= 0 {
            return None;
        }
        let (bits, planes) = (
            GetDeviceCaps(output, BITSPIXEL as i32),
            GetDeviceCaps(output, PLANES as i32),
        );
        let key = (hwnd as usize, slot);
        let old = CACHE.with(|cache| {
            let mut c = cache.borrow_mut();
            let old = c.entries.remove(&key);
            if let Some((ref buffer, _)) = old {
                c.bytes -= buffer.bytes;
            }
            old.map(|(buffer, _)| buffer)
        });
        let buffer = match old {
            Some(buffer)
                if (buffer.width, buffer.height, buffer.bits, buffer.planes)
                    == (width, height, bits, planes) =>
            {
                buffer
            }
            _ => Buffer::new(output, width, height, bits, planes)?,
        };
        let saved = SaveDC(buffer.dc);
        if saved == 0 {
            return None;
        }
        Some(Self {
            buffer: Some(buffer),
            key,
            saved,
        })
    }
    pub fn dc(&self) -> HDC {
        self.buffer.as_ref().unwrap().dc
    }
}
impl Drop for PaintBuffer {
    fn drop(&mut self) {
        let Some(buffer) = self.buffer.take() else {
            return;
        };
        unsafe {
            RestoreDC(buffer.dc, self.saved);
            if IsWindow(self.key.0 as HWND) == 0 {
                return;
            }
        }
        if buffer.bytes > LIMIT {
            return;
        }
        CACHE.with(|cache| {
            let mut c = cache.borrow_mut();
            if let Some((previous, _)) = c.entries.remove(&self.key) {
                c.bytes -= previous.bytes;
            }
            while c.bytes + buffer.bytes > LIMIT || c.entries.len() >= MAX_ENTRIES {
                let Some(key) = c
                    .entries
                    .iter()
                    .min_by_key(|(_, (_, age))| *age)
                    .map(|(key, _)| *key)
                else {
                    break;
                };
                if let Some((removed, _)) = c.entries.remove(&key) {
                    c.bytes -= removed.bytes;
                }
            }
            c.tick = c.tick.wrapping_add(1);
            let tick = c.tick;
            c.bytes += buffer.bytes;
            c.entries.insert(self.key, (buffer, tick));
        });
    }
}
pub fn release(hwnd: HWND) {
    CACHE.with(|cache| {
        let mut c = cache.borrow_mut();
        let keys = c
            .entries
            .keys()
            .filter(|(window, _)| *window == hwnd as usize)
            .copied()
            .collect::<Vec<_>>();
        for key in keys {
            if let Some((buffer, _)) = c.entries.remove(&key) {
                c.bytes -= buffer.bytes;
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::null_mut;
    #[test]
    fn repeated_paints_reuse_resize_replaces_and_release_frees_surfaces() {
        unsafe {
            let class = crate::appearance::wide("STATIC");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                64,
                64,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            assert!(!hwnd.is_null());
            let output = GetDC(hwnd);
            let before = ALLOCATIONS.with(|n| n.get());
            for _ in 0..100 {
                let lease = PaintBuffer::acquire(hwnd, 0, output, 64, 64).unwrap();
                SetWindowOrgEx(lease.dc(), 7, 9, null_mut());
            }
            assert_eq!(ALLOCATIONS.with(|n| n.get()) - before, 1);
            {
                let outer = PaintBuffer::acquire(hwnd, 0, output, 64, 64).unwrap();
                let inner = PaintBuffer::acquire(hwnd, 0, output, 64, 64).unwrap();
                assert_ne!(outer.dc(), inner.dc());
            }
            {
                let lease = PaintBuffer::acquire(hwnd, 0, output, 64, 64).unwrap();
                let mut point = POINT { x: 0, y: 0 };
                GetWindowOrgEx(lease.dc(), &mut point);
                assert_eq!((point.x, point.y), (0, 0));
            }
            for width in [80, 96, 64] {
                drop(PaintBuffer::acquire(hwnd, 0, output, width, 64).unwrap());
                assert_eq!(CACHE.with(|c| c.borrow().entries.len()), 1);
            }
            for slot in 0..140u8 {
                drop(PaintBuffer::acquire(hwnd, slot, output, 64, 64).unwrap());
            }
            assert!(CACHE.with(|c| c.borrow().entries.len()) <= MAX_ENTRIES);
            release(hwnd);
            assert_eq!(CACHE.with(|c| c.borrow().bytes), 0);
            ReleaseDC(hwnd, output);
            DestroyWindow(hwnd);
        }
    }
    #[test]
    fn cache_has_a_fixed_byte_budget() {
        unsafe {
            let class = crate::appearance::wide("STATIC");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                64,
                64,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            let output = GetDC(hwnd);
            for slot in 0..6 {
                drop(PaintBuffer::acquire(hwnd, slot, output, 1920, 1080).unwrap());
            }
            assert!(CACHE.with(|c| c.borrow().bytes) <= LIMIT);
            assert!(CACHE.with(|c| c.borrow().entries.len()) < 6);
            release(hwnd);
            ReleaseDC(hwnd, output);
            DestroyWindow(hwnd);
        }
    }
}
