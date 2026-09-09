fn main() {
    // Default Tauri Windows manifest: requestedExecutionLevel="asInvoker"
    // plus the Common-Controls v6 dependency.
    //
    // Do NOT raise this to `highestAvailable`/`requireAdministrator`. That
    // forces a UAC consent prompt for administrator users on every launch
    // (including autostart at logon and updater relaunch), makes the per-user
    // autostart entry unreliable, blocks single-instance IPC across integrity
    // levels, and makes the test binary unlaunchable from a normal shell
    // (os error 740). Elevation is available on demand through the UI's
    // "Run as Admin" action (platform::restart_elevated).
    tauri_build::build();
}
