// Builds the app icon (assets/PsyAudio.ico) into PsyAudio.exe so Explorer, the taskbar
// and shortcuts show it.
fn main() {
    println!("cargo:rerun-if-changed=assets/PsyAudio.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/PsyAudio.ico");
        res.set("ProductName", "PsyAudio");
        res.set("FileDescription", "PsyAudio");
        res.set("CompanyName", "PsyAudio");
        res.set("LegalCopyright", "");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon not built into the program: {e}");
        }
    }
}
