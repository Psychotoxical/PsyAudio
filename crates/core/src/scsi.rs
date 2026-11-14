//! SCSI commands to the CD drive (Windows SCSI pass-through, Linux SG_IO): the drive's buffer size
//! now; subchannel, ISRC and CD-Text later (step B).

/// The mode page in a MODE SENSE(10) answer: after the 8-byte header and any block descriptors.
pub fn page_of_mode_sense10(resp: &[u8]) -> Option<&[u8]> {
    let descriptors = u16::from_be_bytes([*resp.get(6)?, *resp.get(7)?]) as usize;
    resp.get(8 + descriptors..).filter(|p| !p.is_empty())
}

/// "Buffer Size Supported" (KB) of the CD capabilities page 2Ah: bytes 12–13, big-endian.
pub fn parse_buffer_kb(page_2a: &[u8]) -> Option<u32> {
    if page_2a.len() < 14 || page_2a[0] & 0x3F != 0x2A {
        return None;
    }
    let kb = u16::from_be_bytes([page_2a[12], page_2a[13]]) as u32;
    (kb > 0).then_some(kb)
}

/// How much to read elsewhere so a re-read comes from the disc, not the drive's buffer: the
/// setting when it is not 0 (automatic), else the measured buffer with half again on top (at
/// least 1 MB), else 8 MB.
pub fn flush_kb(override_kb: u32, measured: Option<u32>) -> u32 {
    if override_kb > 0 {
        return override_kb;
    }
    match measured {
        Some(kb) => (kb.saturating_mul(3) / 2).max(1024),
        None => 8192,
    }
}

/// MODE SENSE(10) of one page (current values, no block descriptors).
pub fn mode_sense_page(d: char, page: u8) -> Result<Vec<u8>, String> {
    let cdb = [0x5A, 0x08, page & 0x3F, 0, 0, 0, 0, 0x01, 0x00, 0];
    let resp = command_in(d, &cdb, 256)?;
    page_of_mode_sense10(&resp).map(|p| p.to_vec()).ok_or_else(|| crate::i18n::t("Laufwerk lieferte zu wenig Daten").to_string())
}

/// The drive's buffer in KB as it reports it.
pub fn buffer_kb(d: char) -> Option<u32> {
    mode_sense_page(d, 0x2A).ok().and_then(|p| parse_buffer_kb(&p))
}

/// START STOP UNIT with Start = 0 and LoEj = 0: the disc stops spinning, the tray stays shut.
pub const STOP_UNIT: [u8; 6] = [0x1B, 0, 0, 0, 0x00, 0];

/// Let the disc spin down (it starts again by itself on the next read).
pub fn stop_disc(d: char) -> Result<(), String> {
    command_in(d, &STOP_UNIT, 0).map(|_| ())
}

#[cfg(any(windows, target_os = "linux"))]
fn failed(e: impl std::fmt::Display) -> String {
    crate::i18n::tf(crate::i18n::t("Laufwerksbefehl fehlgeschlagen (Fehler {0})"), &[("0", e.to_string())])
}

/// A command that reads up to `len` bytes from the drive.
#[cfg(windows)]
fn command_in(d: char, cdb: &[u8], len: usize) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const IOCTL_SCSI_PASS_THROUGH_DIRECT: u32 = 0x0004_D014;
    const SCSI_IOCTL_DATA_IN: u8 = 1;

    /// SCSI_PASS_THROUGH_DIRECT, followed by room for the sense data.
    #[repr(C)]
    struct Sptd {
        length: u16,
        scsi_status: u8,
        path_id: u8,
        target_id: u8,
        lun: u8,
        cdb_length: u8,
        sense_info_length: u8,
        data_in: u8,
        data_transfer_length: u32,
        timeout_value: u32,
        data_buffer: *mut core::ffi::c_void,
        sense_info_offset: u32,
        cdb: [u8; 16],
    }
    #[repr(C)]
    struct WithSense {
        sptd: Sptd,
        sense: [u8; 32],
    }

    let name: Vec<u16> = format!("\\\\.\\{d}:").encode_utf16().chain(std::iter::once(0)).collect();
    // pass-through needs a handle opened for reading and writing
    let h = unsafe {
        CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut())
    };
    if h == INVALID_HANDLE_VALUE || h.is_null() {
        return Err(failed(std::io::Error::last_os_error()));
    }
    let mut data = vec![0u8; len];
    let mut cmd = [0u8; 16];
    cmd[..cdb.len()].copy_from_slice(cdb);
    let mut s = WithSense {
        sptd: Sptd {
            length: std::mem::size_of::<Sptd>() as u16,
            scsi_status: 0,
            path_id: 0,
            target_id: 0,
            lun: 0,
            cdb_length: cdb.len() as u8,
            sense_info_length: 32,
            data_in: SCSI_IOCTL_DATA_IN,
            data_transfer_length: len as u32,
            timeout_value: 10,
            data_buffer: data.as_mut_ptr() as *mut _,
            sense_info_offset: std::mem::offset_of!(WithSense, sense) as u32,
            cdb: cmd,
        },
        sense: [0; 32],
    };
    let size = std::mem::size_of::<WithSense>() as u32;
    let mut ret = 0u32;
    let ok = unsafe {
        DeviceIoControl(h, IOCTL_SCSI_PASS_THROUGH_DIRECT, &mut s as *mut _ as *mut _, size, &mut s as *mut _ as *mut _, size, &mut ret, std::ptr::null_mut())
    };
    let err = std::io::Error::last_os_error();
    unsafe {
        CloseHandle(h);
    }
    if ok == 0 {
        return Err(failed(err));
    }
    if s.sptd.scsi_status != 0 {
        return Err(failed(format!("SCSI status {:#04x}", s.sptd.scsi_status)));
    }
    data.truncate((s.sptd.data_transfer_length as usize).min(len));
    Ok(data)
}

/// A command that reads up to `len` bytes from the drive.
#[cfg(target_os = "linux")]
fn command_in(d: char, cdb: &[u8], len: usize) -> Result<Vec<u8>, String> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    const SG_IO: libc::c_ulong = 0x2285;
    const SG_DXFER_FROM_DEV: libc::c_int = -3;

    /// `struct sg_io_hdr`.
    #[repr(C)]
    struct SgIoHdr {
        interface_id: libc::c_int,
        dxfer_direction: libc::c_int,
        cmd_len: u8,
        mx_sb_len: u8,
        iovec_count: u16,
        dxfer_len: u32,
        dxferp: *mut libc::c_void,
        cmdp: *const u8,
        sbp: *mut u8,
        timeout: u32,
        flags: u32,
        pack_id: libc::c_int,
        usr_ptr: *mut libc::c_void,
        status: u8,
        masked_status: u8,
        msg_status: u8,
        sb_len_wr: u8,
        host_status: u16,
        driver_status: u16,
        resid: libc::c_int,
        duration: u32,
        info: u32,
    }

    let f = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(crate::cdrip::drive_name(d)).map_err(failed)?;
    let mut data = vec![0u8; len];
    let mut sense = [0u8; 32];
    let mut h = SgIoHdr {
        interface_id: b'S' as libc::c_int,
        dxfer_direction: SG_DXFER_FROM_DEV,
        cmd_len: cdb.len() as u8,
        mx_sb_len: sense.len() as u8,
        iovec_count: 0,
        dxfer_len: len as u32,
        dxferp: data.as_mut_ptr() as *mut _,
        cmdp: cdb.as_ptr(),
        sbp: sense.as_mut_ptr(),
        timeout: 10_000,
        flags: 0,
        pack_id: 0,
        usr_ptr: std::ptr::null_mut(),
        status: 0,
        masked_status: 0,
        msg_status: 0,
        sb_len_wr: 0,
        host_status: 0,
        driver_status: 0,
        resid: 0,
        duration: 0,
        info: 0,
    };
    if unsafe { libc::ioctl(f.as_raw_fd(), SG_IO as _, &mut h) } < 0 {
        return Err(failed(std::io::Error::last_os_error()));
    }
    if h.status != 0 || h.host_status != 0 || h.driver_status != 0 {
        return Err(failed(format!("SCSI status {:#04x}", h.status)));
    }
    data.truncate(len.saturating_sub(h.resid.max(0) as usize));
    Ok(data)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn command_in(_d: char, _cdb: &[u8], _len: usize) -> Result<Vec<u8>, String> {
    Err(crate::i18n::t("nicht unterstützt").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A MODE SENSE(10) answer: 8-byte header (no block descriptors), then page 2Ah.
    fn answer(buffer_kb: u16) -> Vec<u8> {
        let mut page = vec![0u8; 28];
        page[0] = 0x2A;
        page[1] = 26;
        page[12..14].copy_from_slice(&buffer_kb.to_be_bytes());
        let mut out = vec![0u8; 8];
        out[0..2].copy_from_slice(&((6 + page.len()) as u16).to_be_bytes());
        out.extend(page);
        out
    }

    #[test]
    fn page_follows_header_and_block_descriptors() {
        let a = answer(4096);
        assert_eq!(page_of_mode_sense10(&a).map(|p| p[0]), Some(0x2A));
        let mut with_descriptor = a[..8].to_vec();
        with_descriptor[7] = 8; // 8 bytes of block descriptor
        with_descriptor.extend([0u8; 8]);
        with_descriptor.extend(&a[8..]);
        assert_eq!(page_of_mode_sense10(&with_descriptor).map(|p| p[0]), Some(0x2A));
        assert_eq!(page_of_mode_sense10(&[0u8; 4]), None);
    }

    #[test]
    fn parse_buffer_reads_bytes_12_13() {
        let a = answer(4096);
        assert_eq!(parse_buffer_kb(page_of_mode_sense10(&a).unwrap()), Some(4096));
        let zero = answer(0);
        assert_eq!(parse_buffer_kb(page_of_mode_sense10(&zero).unwrap()), None);
        assert_eq!(parse_buffer_kb(&[0x2A, 2, 0, 0]), None, "too short");
        assert_eq!(parse_buffer_kb(&[0x05; 28]), None, "another page");
    }

    #[test]
    fn stop_is_start_stop_unit_without_eject() {
        assert_eq!(STOP_UNIT, [0x1B, 0, 0, 0, 0x00, 0]);
    }

    #[test]
    fn flush_prefers_override_then_measured() {
        assert_eq!(flush_kb(2048, Some(4096)), 2048);
        assert_eq!(flush_kb(0, Some(4096)), 6144);
        assert_eq!(flush_kb(0, Some(256)), 1024);
        assert_eq!(flush_kb(0, None), 8192);
    }
}
