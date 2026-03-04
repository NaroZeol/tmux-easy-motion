use nix::errno::Errno;
use nix::sys::termios::{self, LocalFlags, SetArg};
use std::os::fd::BorrowedFd;

pub(crate) struct TerminalGuard {
    fd: i32,
    original: termios::Termios,
}

impl TerminalGuard {
    pub(crate) fn setup(fd: i32) -> Result<Option<Self>, String> {
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        let original = match termios::tcgetattr(borrowed) {
            Ok(settings) => settings,
            Err(Errno::ENOTTY) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut new_settings = original.clone();
        new_settings.local_flags.remove(LocalFlags::ICANON);
        new_settings.local_flags.remove(LocalFlags::ECHO);
        termios::tcsetattr(borrowed, SetArg::TCSADRAIN, &new_settings).map_err(|e| e.to_string())?;
        Ok(Some(Self { fd, original }))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let borrowed = unsafe { BorrowedFd::borrow_raw(self.fd) };
        let _ = termios::tcsetattr(borrowed, SetArg::TCSADRAIN, &self.original);
    }
}
