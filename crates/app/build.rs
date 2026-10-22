fn main() {
    // Embed the app icon: resource 1 is the exe icon and the tray icon.
    //
    // The application manifest comes from GPUI (`gpui.manifest.xml`):
    // `asInvoker`, per-monitor DPI v2, Common Controls v6, and SegmentHeap.
    // Keep the execution level at `asInvoker`. A higher level forces a UAC
    // prompt at every launch (including autostart at logon), makes the
    // per-user autostart entry unreliable, blocks single-instance messages
    // across integrity levels, and makes test binaries unlaunchable from a
    // normal shell (os error 740). Elevation stays on demand through
    // `platform::restart_elevated`.
    println!("cargo:rerun-if-changed=assets/hypertype.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    embed_resource::compile("assets/hypertype.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
