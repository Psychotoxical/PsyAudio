//! macOS system calls: optical drives in the I/O Registry, CD reads through the system's
//! CD-DA interface (`/dev/rdiskN` ioctls).

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use io_kit_sys::types::io_object_t;
use io_kit_sys::{
    kIOMasterPortDefault, kIORegistryIterateRecursively, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperty, IORegistryEntrySearchCFProperty,
    IOServiceGetMatchingServices, IOServiceMatching,
};
use std::ffi::CString;

/// An optical drive as the I/O Registry lists it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpticalDrive {
    /// BSD name of the inserted disc ("disk2"), `None` without a disc.
    pub bsd: Option<String>,
    pub vendor: String,
    pub product: String,
}

/// Every optical drive (CD, DVD and Blu-ray drives all derive from IOCDBlockStorageDevice).
pub fn optical_drives() -> Vec<OpticalDrive> {
    let mut out = Vec::new();
    let class = CString::new("IOCDBlockStorageDevice").expect("no NUL");
    unsafe {
        let matching = IOServiceMatching(class.as_ptr());
        if matching.is_null() {
            return out;
        }
        let mut iter = 0;
        // IOServiceGetMatchingServices consumes `matching`
        if IOServiceGetMatchingServices(kIOMasterPortDefault, matching, &mut iter) != 0 {
            return out;
        }
        loop {
            let service = IOIteratorNext(iter);
            if service == 0 {
                break;
            }
            out.push(describe(service));
            IOObjectRelease(service);
        }
        IOObjectRelease(iter);
    }
    out
}

unsafe fn describe(service: io_object_t) -> OpticalDrive {
    let mut d = OpticalDrive::default();
    if let Some(chars) = property(service, "Device Characteristics").and_then(|v| v.downcast::<CFDictionary>()) {
        let text = |key: &str| {
            chars
                .find(CFString::new(key).as_CFTypeRef())
                .map(|v| unsafe { CFType::wrap_under_get_rule(*v) })
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string())
                .unwrap_or_default()
        };
        d.vendor = text("Vendor Name");
        d.product = text("Product Name");
    }
    let key = CFString::new("BSD Name");
    let found = IORegistryEntrySearchCFProperty(service, c"IOService".as_ptr(), key.as_concrete_TypeRef(), std::ptr::null(), kIORegistryIterateRecursively);
    if !found.is_null() {
        d.bsd = CFType::wrap_under_create_rule(found).downcast::<CFString>().map(|s| s.to_string());
    }
    d
}

unsafe fn property(entry: io_object_t, name: &str) -> Option<CFType> {
    let key = CFString::new(name);
    let v = IORegistryEntryCreateCFProperty(entry, key.as_concrete_TypeRef(), std::ptr::null(), 0);
    (!v.is_null()).then(|| CFType::wrap_under_create_rule(v))
}

mod da {
    use core_foundation::base::CFAllocatorRef;
    use core_foundation::runloop::CFRunLoopRef;
    use core_foundation::string::CFStringRef;
    use std::ffi::{c_char, c_void};

    pub type DASessionRef = *const c_void;
    pub type DADiskRef = *const c_void;
    pub type DADissenterRef = *const c_void;
    pub type Callback = extern "C" fn(DADiskRef, DADissenterRef, *mut c_void);
    pub const UNMOUNT_WHOLE: u32 = 0x0000_0001;

    #[link(name = "DiskArbitration", kind = "framework")]
    extern "C" {
        pub fn DASessionCreate(alloc: CFAllocatorRef) -> DASessionRef;
        pub fn DASessionScheduleWithRunLoop(session: DASessionRef, run_loop: CFRunLoopRef, mode: CFStringRef);
        pub fn DASessionUnscheduleFromRunLoop(session: DASessionRef, run_loop: CFRunLoopRef, mode: CFStringRef);
        pub fn DADiskCreateFromBSDName(alloc: CFAllocatorRef, session: DASessionRef, name: *const c_char) -> DADiskRef;
        pub fn DADiskUnmount(disk: DADiskRef, options: u32, callback: Callback, context: *mut c_void);
        pub fn DADiskEject(disk: DADiskRef, options: u32, callback: Callback, context: *mut c_void);
        pub fn DADissenterGetStatus(dissenter: DADissenterRef) -> i32;
        pub fn DADissenterGetStatusString(dissenter: DADissenterRef) -> CFStringRef;
    }
}

/// What a DiskArbitration call answered: `None` while it runs.
type Answer = Option<Result<(), String>>;

extern "C" fn on_done(_disk: da::DADiskRef, dissenter: da::DADissenterRef, ctx: *mut std::ffi::c_void) {
    let answer = unsafe { &mut *(ctx as *mut Answer) };
    *answer = Some(if dissenter.is_null() {
        Ok(())
    } else {
        let status = unsafe { da::DADissenterGetStatus(dissenter) };
        let text = unsafe { da::DADissenterGetStatusString(dissenter) };
        Err(if text.is_null() { format!("{status:#x}") } else { unsafe { CFString::wrap_under_get_rule(text) }.to_string() })
    });
    core_foundation::runloop::CFRunLoop::get_current().stop();
}

/// Unmounts the disc (`cddafs` mounts every audio CD) and ejects it, waiting at most 10 s per step.
pub fn eject(bsd: &str) -> Result<(), String> {
    arbitrate(bsd, true)
}

/// Unmounts the disc but leaves it in the drive (macOS mounts it again by itself later).
pub fn unmount(bsd: &str) -> Result<(), String> {
    arbitrate(bsd, false)
}

fn arbitrate(bsd: &str, then_eject: bool) -> Result<(), String> {
    use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
    let failed = |e: String| crate::i18n::tf(crate::i18n::t("Laufwerksbefehl fehlgeschlagen (Fehler {0})"), &[("0", e)]);
    let name = CString::new(bsd).map_err(|e| failed(e.to_string()))?;
    unsafe {
        let session = da::DASessionCreate(std::ptr::null());
        if session.is_null() {
            return Err(failed("DiskArbitration".into()));
        }
        let session = CFType::wrap_under_create_rule(session as _);
        let disk = da::DADiskCreateFromBSDName(std::ptr::null(), session.as_CFTypeRef() as _, name.as_ptr());
        if disk.is_null() {
            return Err(failed(bsd.to_string()));
        }
        let disk = CFType::wrap_under_create_rule(disk as _);
        let run_loop = CFRunLoop::get_current();
        da::DASessionScheduleWithRunLoop(session.as_CFTypeRef() as _, run_loop.as_concrete_TypeRef(), kCFRunLoopDefaultMode);
        let step = |start: &dyn Fn(*mut std::ffi::c_void)| -> Result<(), String> {
            let mut answer: Answer = None;
            start(&mut answer as *mut Answer as *mut std::ffi::c_void);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while answer.is_none() && std::time::Instant::now() < deadline {
                CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, std::time::Duration::from_millis(250), true);
            }
            answer.unwrap_or_else(|| Err(crate::i18n::t("Zeitüberschreitung").to_string())).map_err(failed)
        };
        let d = disk.as_CFTypeRef() as da::DADiskRef;
        let result = step(&|ctx| da::DADiskUnmount(d, da::UNMOUNT_WHOLE, on_done, ctx)).and_then(|()| {
            if then_eject {
                step(&|ctx| da::DADiskEject(d, 0, on_done, ctx))
            } else {
                Ok(())
            }
        });
        da::DASessionUnscheduleFromRunLoop(session.as_CFTypeRef() as _, run_loop.as_concrete_TypeRef(), kCFRunLoopDefaultMode);
        result
    }
}

extern "C" {
    fn psy_scsi_command_in(index: u32, cdb: *const u8, cdb_len: u32, buf: *mut u8, len: u32, got: *mut u32) -> i32;
}

/// A SCSI command that reads up to `len` bytes from drive `index` (order of [`optical_drives`]).
/// macOS hands out the drive only while no disc is mounted: a mounted disc is unmounted first
/// and comes back by itself afterwards, which this waits for (at most 20 s).
pub fn scsi_command_in(index: usize, cdb: &[u8], len: usize) -> Result<Vec<u8>, String> {
    let failed = |e: String| crate::i18n::tf(crate::i18n::t("Laufwerksbefehl fehlgeschlagen (Fehler {0})"), &[("0", e)]);
    let disc = optical_drives().into_iter().nth(index).ok_or_else(|| failed("no drive".into()))?.bsd;
    if let Some(bsd) = &disc {
        unmount(bsd)?;
    }
    let mut data = vec![0u8; len];
    let mut got = 0u32;
    let status = unsafe { psy_scsi_command_in(index as u32, cdb.as_ptr(), cdb.len() as u32, data.as_mut_ptr(), len as u32, &mut got) };
    if disc.is_some() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        // the disc is listed a moment before its device file can be opened
        let ready = || optical_drives().into_iter().nth(index).and_then(|o| o.bsd).is_some_and(|b| std::fs::File::open(format!("/dev/r{b}")).is_ok());
        while !ready() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    if status != 0 {
        return Err(failed(format!("{status:#x}")));
    }
    data.truncate(got as usize);
    Ok(data)
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(kind: core_foundation::string::CFStringRef, level: u32, name: core_foundation::string::CFStringRef, id: *mut u32) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

/// The assertion that keeps the Mac awake and how many jobs want it (a rip and a conversion can
/// run at once; Windows keeps one request per thread, this counts them).
static SLEEP_ASSERTION: std::sync::Mutex<(u32, Option<u32>)> = std::sync::Mutex::new((0, None));

/// Keeps the Mac from idle sleep while converting or ripping (the display may still sleep); every
/// `true` needs its `false`, the assertion goes when the last job ends.
pub fn power_assertion(on: bool) {
    let mut state = SLEEP_ASSERTION.lock().unwrap_or_else(|e| e.into_inner());
    let (count, held) = &mut *state;
    if on {
        *count += 1;
        if held.is_none() {
            let kind = CFString::new("PreventUserIdleSystemSleep");
            let name = CFString::new("PsyAudio");
            let mut id = 0u32;
            // kIOPMAssertionLevelOn = 255
            if unsafe { IOPMAssertionCreateWithName(kind.as_concrete_TypeRef(), 255, name.as_concrete_TypeRef(), &mut id) } == 0 {
                *held = Some(id);
            }
        }
    } else {
        *count = count.saturating_sub(1);
        if *count == 0 {
            if let Some(id) = held.take() {
                unsafe { IOPMAssertionRelease(id) };
            }
        }
    }
}

/// Whether PsyAudio keeps the Mac awake right now.
pub fn power_assertion_held() -> bool {
    SLEEP_ASSERTION.lock().unwrap_or_else(|e| e.into_inner()).1.is_some()
}

/// `dk_cd_read_t` from `<IOKit/storage/IOCDMediaBSDClient.h>` (LP64).
#[repr(C)]
#[derive(Default)]
pub struct CdRead {
    pub offset: u64,
    pub sector_area: u8,
    pub sector_type: u8,
    pub reserved: [u8; 10],
    pub buffer_length: u32,
    pub buffer: usize,
}

/// `dk_cd_read_toc_t`.
#[repr(C)]
#[derive(Default)]
pub struct CdReadToc {
    pub format: u8,
    pub format_as_time: u8,
    pub reserved: [u8; 5],
    pub address: u8,
    pub reserved2: [u8; 6],
    pub buffer_length: u16,
    pub buffer: usize,
}

/// `_IOWR('d', num, size)` as `<sys/ioccom.h>` builds it.
const fn iowr(num: u64, size: usize) -> u64 {
    0xC000_0000 | ((size as u64 & 0x1FFF) << 16) | ((b'd' as u64) << 8) | num
}

pub const DKIOCCDREAD: u64 = iowr(96, std::mem::size_of::<CdRead>());
pub const DKIOCCDREADTOC: u64 = iowr(100, std::mem::size_of::<CdReadToc>());
pub const CD_SECTOR_AREA_USER: u8 = 0x10;
pub const CD_SECTOR_TYPE_CDDA: u8 = 0x01;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Values printed from the SDK headers (clang, x86_64 and arm64 alike).
    #[test]
    fn layouts_and_codes_match_the_headers() {
        assert_eq!(size_of::<CdRead>(), 32);
        assert_eq!(offset_of!(CdRead, sector_area), 8);
        assert_eq!(offset_of!(CdRead, sector_type), 9);
        assert_eq!(offset_of!(CdRead, buffer_length), 20);
        assert_eq!(offset_of!(CdRead, buffer), 24);
        assert_eq!(size_of::<CdReadToc>(), 24);
        assert_eq!(offset_of!(CdReadToc, address), 7);
        assert_eq!(offset_of!(CdReadToc, buffer_length), 14);
        assert_eq!(offset_of!(CdReadToc, buffer), 16);
        assert_eq!(DKIOCCDREAD, 0xc020_6460);
        assert_eq!(DKIOCCDREADTOC, 0xc018_6464);
    }

    /// Held while any job wants it (a rip and a conversion at once): released when the last one
    /// ends, and an extra "off" does not go below zero.
    #[test]
    fn sleep_assertion_is_held_until_the_last_job_ends() {
        power_assertion(true);
        power_assertion(true);
        assert!(power_assertion_held());
        power_assertion(false);
        assert!(power_assertion_held(), "the other job still runs");
        power_assertion(false);
        assert!(!power_assertion_held());
        power_assertion(false);
        power_assertion(true);
        assert!(power_assertion_held());
        power_assertion(false);
        assert!(!power_assertion_held());
    }

    /// Listing drives works without one attached.
    #[test]
    fn listing_drives_does_not_fail() {
        let _ = optical_drives();
    }
}
