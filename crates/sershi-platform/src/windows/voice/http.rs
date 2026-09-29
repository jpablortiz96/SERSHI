//! HTTPS GET over WinHTTP, for model downloads only.
//!
//! Uses Windows' own TLS stack and proxy configuration (no extra TLS crate).
//! Only `https://` URLs from SERSHI's fixed model catalog are opened;
//! redirects may never downgrade to HTTP; TLS 1.2 or newer is required.

use std::ffi::c_void;
use std::io::{self, Read};

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2, WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP,
    WINHTTP_OPTION_SECURE_PROTOCOLS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpReadData,
    WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts,
};
use windows::core::{PCWSTR, w};

use super::super::util::wide;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    InvalidUrl,
    Network,
    Status(u32),
}

/// One HTTPS GET response body, readable as a stream.
#[derive(Debug)]
pub struct HttpsGet {
    session: *mut c_void,
    connection: *mut c_void,
    request: *mut c_void,
}

// SAFETY: WinHTTP handles are thread-agnostic; the struct is used by one
// thread at a time (it is moved, never shared).
unsafe impl Send for HttpsGet {}

fn set_option(handle: *mut c_void, option: u32, value: u32) -> bool {
    let bytes = value.to_le_bytes();
    // SAFETY: `handle` is a live WinHTTP handle; the buffer is a u32 option.
    unsafe { WinHttpSetOption(Some(handle.cast_const()), option, Some(&bytes)) }.is_ok()
}

impl HttpsGet {
    pub fn open(url: &str) -> Result<Self, HttpError> {
        let rest = url.strip_prefix("https://").ok_or(HttpError::InvalidUrl)?;
        let (host, path) = rest.split_once('/').ok_or(HttpError::InvalidUrl)?;
        if host.is_empty()
            || !host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c))
        {
            return Err(HttpError::InvalidUrl);
        }
        let mut get = HttpsGet {
            session: std::ptr::null_mut(),
            connection: std::ptr::null_mut(),
            request: std::ptr::null_mut(),
        };
        // SAFETY: plain WinHTTP calls on handles owned by `get`, which closes
        // them in `Drop` (in reverse order) whatever happens below.
        unsafe {
            get.session = WinHttpOpen(
                w!("SERSHI"),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            );
            if get.session.is_null() {
                return Err(HttpError::Network);
            }
            // Resolve, connect, send: 15 s; each receive: 60 s.
            WinHttpSetTimeouts(get.session, 15_000, 15_000, 15_000, 60_000)
                .map_err(|_| HttpError::Network)?;
            let tls12_13 =
                WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2 | WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3;
            // Older Windows 10 builds lack TLS 1.3 in WinHTTP.
            if !set_option(get.session, WINHTTP_OPTION_SECURE_PROTOCOLS, tls12_13)
                && !set_option(
                    get.session,
                    WINHTTP_OPTION_SECURE_PROTOCOLS,
                    WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2,
                )
            {
                return Err(HttpError::Network);
            }
            if !set_option(
                get.session,
                WINHTTP_OPTION_REDIRECT_POLICY,
                WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP,
            ) {
                return Err(HttpError::Network);
            }
            let host = wide(host);
            get.connection = WinHttpConnect(
                get.session,
                PCWSTR(host.as_ptr()),
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            );
            if get.connection.is_null() {
                return Err(HttpError::Network);
            }
            let path = wide(format!("/{path}"));
            get.request = WinHttpOpenRequest(
                get.connection,
                w!("GET"),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            );
            if get.request.is_null() {
                return Err(HttpError::Network);
            }
            WinHttpSendRequest(get.request, None, None, 0, 0, 0).map_err(|_| HttpError::Network)?;
            WinHttpReceiveResponse(get.request, std::ptr::null_mut())
                .map_err(|_| HttpError::Network)?;
            let mut status: u32 = 0;
            let mut len = std::mem::size_of::<u32>() as u32;
            windows::Win32::Networking::WinHttp::WinHttpQueryHeaders(
                get.request,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut status as *mut u32).cast()),
                &mut len,
                std::ptr::null_mut(),
            )
            .map_err(|_| HttpError::Network)?;
            if status != 200 {
                return Err(HttpError::Status(status));
            }
        }
        Ok(get)
    }
}

impl Read for HttpsGet {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let want = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut read: u32 = 0;
        // SAFETY: `buf` is valid for `want` bytes; the request handle is live.
        unsafe { WinHttpReadData(self.request, buf.as_mut_ptr().cast(), want, &mut read) }
            .map_err(|_| io::Error::other("download interrupted"))?;
        Ok(read as usize)
    }
}

impl Drop for HttpsGet {
    fn drop(&mut self) {
        for handle in [self.request, self.connection, self.session] {
            if !handle.is_null() {
                // SAFETY: each handle was opened by WinHTTP and is closed once.
                let _ = unsafe { WinHttpCloseHandle(handle) };
            }
        }
    }
}
