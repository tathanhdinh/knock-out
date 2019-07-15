#![feature(lang_items)]
#![no_std]

use core::panic::PanicInfo;
use core::result;
use core::fmt::{self, Write};

mod parrot;

use parrot::*;

#[lang = "eh_personality"]
#[no_mangle]
pub extern fn eh_personality() {}
#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern fn eh_unwind_resume() {}
#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

extern "C" {
    static owner: *const u8;
    static cdev_ptr: *mut u8;
    static fops_ptr: *mut u8;
    static parrot_owner_ptr: *mut *const u8;
    static parrot_read_ptr: *mut extern "C" fn(*mut u8, *mut u8, u32, *const u32) -> i32;
    static parrot_open_ptr: *mut extern "C" fn(*mut u8, *mut u8) -> i32;
    static parrot_release_ptr: *mut extern "C" fn(*mut u8, *mut u8) -> i32;
    fn printk(msg: *const u8);
    fn alloc_chrdev_region(first: *const u32, first_minor: u32, count: u32, name: *const u8) -> i32;
    fn unregister_chrdev_region(first: u32, count: u32);
	#[inline]
    fn copy_to_user_ffi(to: *mut u8, from: *const u8, count: u64) -> u64;
    fn cdev_init(cdev: *mut u8, fops: *const u8);
    fn cdev_add(cdev: *mut u8, dev: u32, count: u32) -> i32;
    fn cdev_del(cdev: *mut u8);
    fn msleep(msecs: u64);
}

const FRAMES: [&str; 10] = [FRAME0, FRAME1, FRAME2, FRAME3, FRAME4, FRAME5, FRAME6,
                            FRAME7, FRAME8, FRAME9];
static mut FRAME_COUNTER: u8 = 0;

#[no_mangle]
extern "C" fn parrot_read(_file: *mut u8, buf: *mut u8, _count: u32, _offset: *const u32) -> i32 {
    let frame = FRAMES.get(unsafe { FRAME_COUNTER } as usize).unwrap_or(&"");
    ParrotSafe::copy_to_user_ffi_safe(buf, frame.as_bytes());
    unsafe {
        FRAME_COUNTER = FRAME_COUNTER.wrapping_add(1) % 10;
        // Yes, this is terrible
        msleep(50);
    }
    frame.len() as i32
}

#[no_mangle]
extern "C" fn parrot_open(_inode: *mut u8, _file: *mut u8) -> i32 {
    0
}

#[no_mangle]
extern "C" fn parrot_release(_inode: *mut u8, _file: *mut u8) -> i32 {
    0
}

enum ParrorError {
    CharDevAdd,
    CharDevRegionAlloc,
}

type Result<T> = result::Result<T, ParrorError>;

impl fmt::Display for ParrorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ParrorError::*;
        match self {
            CharDevAdd => {
                write!(f, "{}", "Failed to add char dev\0")
            },

            CharDevRegionAlloc => {
                write!(f, "{}", "Failed to allocate char device region\0")
            }
        }
    }
}

#[derive(Default)]
struct KernLog;

impl fmt::Write for KernLog {
    fn write_str(&mut self, s: &str) -> result::Result<(), fmt::Error> {
        unsafe { printk(s.as_ptr()); }
        Ok(())
    }
}

static mut KERNEL_LOG: KernLog = KernLog;

struct ParrotSafe {
    dev: u32,
    count: u32,
}

impl ParrotSafe {
    #[inline]
    fn owner() -> *const u8 {
        unsafe { owner }
    }

    #[inline]
    fn cdev_ptr() -> *mut u8 {
        unsafe { cdev_ptr }
    }

    #[inline]
    fn fops_ptr() -> *mut u8 {
        unsafe { fops_ptr }
    }

    #[inline]
    fn set_fops_safe(read: extern "C" fn(*mut u8, *mut u8, u32, *const u32) -> i32,
                open: extern "C" fn(*mut u8, *mut u8) -> i32,
                release: extern "C" fn(*mut u8, *mut u8) -> i32) {
        unsafe {
            parrot_owner_ptr.write(Self::owner());
            parrot_read_ptr.write(read);
            parrot_open_ptr.write(open);
            parrot_release_ptr.write(release);
        }
    }

    #[inline]
    fn alloc_chrdev_region_safe(&mut self, first_minor: u32, count: u32, name: &'static str) -> i32 {
        self.count = count;
        unsafe { alloc_chrdev_region(&mut self.dev as *mut u32, first_minor, self.count, name.as_ptr()) }
    }

    #[inline]
    fn unregister_chrdev_region_safe(&mut self) {
        unsafe { unregister_chrdev_region(self.dev, self.count) }
    }

    #[inline]
    fn copy_to_user_ffi_safe(to: *mut u8, from: &[u8]) -> u64 {
        unsafe { copy_to_user_ffi(to, from.as_ptr(), from.len() as u64) }
    }

    #[inline]
    fn cdev_init_safe(&mut self) {
        unsafe { cdev_init(Self::cdev_ptr(), Self::fops_ptr()) }
    }

    #[inline]
    fn cdev_add_safe(&mut self) -> Result<()> {
        let rc = unsafe { cdev_add(Self::cdev_ptr(), self.dev, self.count) };
        if rc == 0 {
            Ok(())
        } else {
            Err(ParrorError::CharDevAdd)
        }
    }

    #[inline]
    fn cdev_del_safe(&mut self) {
        unsafe { cdev_del(Self::cdev_ptr()) }
    }

    fn new() -> Result<Self> {
        let mut psafe = ParrotSafe { dev: 0, count: 0, };
        Self::set_fops_safe(parrot_read, parrot_open, parrot_release);
        if psafe.alloc_chrdev_region_safe(0, 1, "parrot\0") != 0 {
            Err(ParrorError::CharDevRegionAlloc)
        } else {
            psafe.cdev_init_safe();
            psafe.cdev_add_safe()?;
            Ok(psafe)
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        self.unregister_chrdev_region_safe();
        self.cdev_del_safe();
        Ok(())
    }
}

static mut GLOBAL_STATE: Option<ParrotSafe> = None;

#[no_mangle]
#[link_section = ".text"]
pub extern "C" fn init_module() -> i32 {
    let parrot_safe = match ParrotSafe::new() {
        Ok(ps) => ps,
        Err(e) => {
            unsafe { write!(KERNEL_LOG, "{}", e).unwrap() };
            return -1;
        }
    };
    unsafe { GLOBAL_STATE = Some(parrot_safe) };
    0
}

#[no_mangle]
#[link_section = ".text"]
pub extern "C" fn cleanup_module() {
    unsafe {
        match GLOBAL_STATE {
            Some(ref mut ps) => {
                match ps.cleanup() {
                    Ok(_) => (),
                    Err(e) => {
                        write!(KERNEL_LOG, "{}", e).unwrap();
                    }
                }
            }
            None => (),
        }
    }
}
