//! Fixed worker spawn with zero inherited Windows handles.
//!
//! Stable Command inherits all inheritable Windows handles, including the
//! caller's captured output pipe even when child stdio uses NUL. Use the
//! native no-inheritance contract for this one fixed background child.
use std::{
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        CreateProcessW, TerminateProcess, WaitForSingleObject, CREATE_BREAKAWAY_FROM_JOB,
        CREATE_NO_WINDOW, PROCESS_INFORMATION, STARTUPINFOW,
    },
};

pub(super) struct Worker {
    process: OwnedHandle,
    identifier: u32,
}

impl Worker {
    pub(super) fn spawn(executable: &Path) -> io::Result<Self> {
        let application: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // Exact executable, including spaces/Unicode, plus fixed arguments.
        // No shell, caller-supplied arguments or global inheritance mutation.
        let mut command = vec![b'"' as u16];
        command.extend(executable.as_os_str().encode_wide());
        command.extend("\" engine supervise".encode_utf16());
        command.push(0);
        let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
        startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: UTF-16 buffers stay live; command is writable; structures
        // have valid size/zero optional fields. FALSE passes no pipe/lock/job
        // handles. Null environment inherits profile values, never handles.
        // Failed breakaway is an error, with no unsafe lifetime fallback.
        let success = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB,
                std::ptr::null(),
                std::ptr::null(),
                &startup,
                &mut process,
            )
        };
        if success == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful CreateProcess returns fresh owned handles.
        unsafe {
            CloseHandle(process.hThread);
        }
        Ok(Self {
            process: unsafe { OwnedHandle::from_raw_handle(process.hProcess) },
            identifier: process.dwProcessId,
        })
    }

    pub(super) fn id(&self) -> u32 {
        self.identifier
    }

    pub(super) fn has_exited(&self) -> io::Result<bool> {
        // SAFETY: valid owned process handle; zero timeout never blocks.
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }

    pub(super) fn stop_unacknowledged(&self) -> io::Result<()> {
        // Exact newly spawned child handle, before acknowledgment; no PID
        // lookup, external worker, distro or app container is terminated.
        if self.has_exited()? {
            return Ok(());
        }
        if unsafe { TerminateProcess(self.process.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 5_000) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "New worker exit was not acknowledged",
            )),
            _ => Err(io::Error::last_os_error()),
        }
    }
}
