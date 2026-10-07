//! Deliver blessed's termination signals through a self-pipe to the UI loop.
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicI32, Ordering};

static WRITE_FD: AtomicI32 = AtomicI32::new(-1);
extern "C" fn notify(_signal: libc::c_int) {
    #[cfg(target_os = "macos")]
    let errno = unsafe { libc::__error() };
    #[cfg(not(target_os = "macos"))]
    let errno = unsafe { libc::__errno_location() };
    let saved_errno = unsafe { *errno };
    let fd = WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = 0u8;
        // write is async-signal-safe; a full nonblocking pipe already has a wakeup.
        unsafe {
            libc::write(fd, (&byte as *const u8).cast(), 1);
        }
    }
    unsafe {
        *errno = saved_errno;
    }
}

pub(crate) struct Guard {
    saved: Vec<(i32, libc::sigaction)>,
    writer: UnixStream,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Guard {
    pub(crate) fn install(on_signal: impl Fn() + Send + 'static) -> io::Result<Self> {
        let (mut reader, writer) = UnixStream::pair()?;
        writer.set_nonblocking(true)?;
        let mut guard = Self {
            saved: Vec::new(),
            writer,
            reader: None,
        };
        WRITE_FD.store(guard.writer.as_raw_fd(), Ordering::Relaxed);
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGQUIT] {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = notify as usize;
            action.sa_flags = libc::SA_RESTART;
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
                return Err(io::Error::last_os_error());
            }
            guard.saved.push((signal, previous));
        }
        guard.reader = Some(std::thread::spawn(move || {
            let mut bytes = [0u8; 64];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => return,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return,
                    Ok(count) => {
                        if bytes[..count].contains(&1) {
                            return;
                        }
                        on_signal();
                    }
                }
            }
        }));
        Ok(guard)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        for (signal, action) in &self.saved {
            unsafe {
                libc::sigaction(*signal, action, std::ptr::null_mut());
            }
        }
        WRITE_FD.store(-1, Ordering::Relaxed);
        // A blocking write is safe here, outside a handler, and the reader drains it.
        let _ = self.writer.set_nonblocking(false);
        let _ = self.writer.write_all(&[1]);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
