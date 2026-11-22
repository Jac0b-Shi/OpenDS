//! 测量用分配计数器：只在 `cfg(test)` 下挂成全局分配器。
//!
//! 基准测试需要区分「省了多少次分配」与「省了多少时间」。计数用线程局部变量，
//! 这样 `cargo test` 并行跑时各测试互不干扰；`const` 初始化的 `Cell`
//! 保证分配器内部记账不会反过来触发分配。

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

pub struct Counting;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    // `try_with` 覆盖线程销毁阶段：那时 TLS 已经不可访问。
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        }
    });
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// 在计数开启的状态下执行 `f`，返回结果与本次分配的字节块个数。
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ENABLED.with(|enabled| enabled.set(true));
    COUNT.with(|count| count.set(0));
    let value = f();
    ENABLED.with(|enabled| enabled.set(false));
    let count = COUNT.with(|count| count.get());
    (value, count)
}
