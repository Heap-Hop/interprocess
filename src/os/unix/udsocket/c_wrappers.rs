use {
    super::imports::*,
    std::{
        ffi::c_void,
        io,
        mem::{size_of, size_of_val, zeroed},
        net::Shutdown,
        ptr,
    },
};

pub(super) fn create_uds(ty: c_int) -> io::Result<FdOps> {
    let (success, fd) = unsafe {
        let result = libc::socket(AF_UNIX, ty, 0);
        (result != -1, result)
    };
    if success {
        let fdops = unsafe {
            // SAFETY: we just created this descriptor
            FdOps::from_raw_fd(fd)
        };
        Ok(fdops)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Binds the specified Ud-socket file descriptor to the given address.
///
/// # Safety
/// `addr` must be properly null-terminated.
pub(super) unsafe fn bind(fd: &FdOps, addr: &sockaddr_un) -> io::Result<()> {
    let success = unsafe {
        libc::bind(
            fd.0,
            // Double cast because you cannot cast a reference to a pointer of arbitrary type
            // but you can cast any narrow pointer to any other narrow pointer
            addr as *const _ as *const sockaddr,
            size_of::<sockaddr_un>() as u32,
        ) != -1
    };
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Connects the specified Ud-socket file descriptor to the given address.
///
/// # Safety
/// `addr` must be properly null-terminated.
pub(super) unsafe fn connect(fd: &FdOps, addr: &sockaddr_un) -> io::Result<()> {
    let success = unsafe {
        libc::connect(
            fd.0,
            addr as *const _ as *const _,
            size_of::<sockaddr_un>() as u32,
        ) != 1
    };
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn listen(fd: &FdOps, backlog: c_int) -> io::Result<()> {
    let success = unsafe { libc::listen(fd.0, backlog) != -1 };
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn set_passcred(fd: &FdOps, passcred: bool) -> io::Result<()> {
    #[cfg(uds_scm_credentials)]
    {
        let passcred = passcred as c_int;
        let success = unsafe {
            libc::setsockopt(
                fd.0,
                SOL_SOCKET,
                SO_PASSCRED,
                &passcred as *const _ as *const _,
                size_of_val(&passcred) as u32,
            ) != -1
        };
        if success {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(uds_scm_credentials))]
    {
        let _ = fd;
        Ok(())
    }
}
#[cfg(uds_peercred)]
pub(super) fn get_peer_ucred(fd: &FdOps) -> io::Result<ucred> {
    let mut cred: ucred = unsafe {
        // SAFETY: it's safe for the ucred structure to be zero-initialized, since
        // it only contains integers
        zeroed()
    };
    let mut cred_len = size_of::<ucred>() as socklen_t;
    let success = unsafe {
        libc::getsockopt(
            fd.0,
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut cred_len as *mut _,
        )
    } != -1;
    if success {
        Ok(cred)
    } else {
        Err(io::Error::last_os_error())
    }
}
pub(super) fn set_nonblocking(fd: &FdOps, nonblocking: bool) -> io::Result<()> {
    let (old_flags, success) = unsafe {
        // SAFETY: nothing too unsafe about this function. One thing to note is that we're passing
        // it a null pointer, which is, for some reason, required yet ignored for F_GETFL.
        let result = libc::fcntl(fd.0, F_GETFL, ptr::null::<c_void>());
        (result, result != -1)
    };
    if !success {
        return Err(io::Error::last_os_error());
    }
    let new_flags = if nonblocking {
        old_flags | O_NONBLOCK
    } else {
        // Inverting the O_NONBLOCK value sets all the bits in the flag set to 1 except for the
        // nonblocking flag, which clears the flag when ANDed.
        old_flags & !O_NONBLOCK
    };
    let success = unsafe {
        // SAFETY: new_flags is a c_int, as documented in the manpage.
        libc::fcntl(fd.0, F_SETFL, new_flags)
    } != -1;
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
pub(super) fn get_nonblocking(fd: &FdOps) -> io::Result<bool> {
    let flags = unsafe {
        // SAFETY: exactly the same as above.
        libc::fcntl(fd.0, F_GETFL, ptr::null::<c_void>())
    };
    if flags != -1 {
        Ok(flags & O_NONBLOCK != 0)
    } else {
        Err(io::Error::last_os_error())
    }
}
pub(super) fn shutdown(fd: &FdOps, how: Shutdown) -> io::Result<()> {
    let how = match how {
        Shutdown::Read => SHUT_RD,
        Shutdown::Write => SHUT_WR,
        Shutdown::Both => SHUT_RDWR,
    };
    let success = unsafe { libc::shutdown(fd.0, how) != -1 };
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
