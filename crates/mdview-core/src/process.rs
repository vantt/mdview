//! Cross-platform process-spawn helpers shared by every crate that launches
//! a detached daemon (`mdview` binary today, `mdview-desktop` next).

/// Apply the platform detach settings to `cmd` so a spawned child outlives its
/// spawner: a new session on Unix (`setsid`), a detached console + new process
/// group on Windows. Extracted from `spawn_daemon_detached` so the detach itself
/// is testable without launching the full daemon.
pub fn apply_detach(cmd: &mut std::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid() is async-signal-safe and is the only call made in the
        // forked child before exec. It puts the child in its own new session (as
        // session leader), detaching it from the spawner's controlling terminal
        // and process group so neither a SIGHUP on session close nor a
        // process-group-directed signal can reach it.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS: no inherited console. CREATE_NEW_PROCESS_GROUP: the
        // daemon does not receive Ctrl+C/Ctrl+Break sent to the spawner's group.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
}
