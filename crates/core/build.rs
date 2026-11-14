//! macOS: SCSI commands to the CD drive go through Apple's SCSITaskLib, a C plug-in interface;
//! a small C file wraps it (`src/macos_scsi.c`). Other systems build nothing here.

fn main() {
    println!("cargo:rerun-if-changed=src/macos_scsi.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new().file("src/macos_scsi.c").warnings(true).compile("psyaudio_macos_scsi");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}
