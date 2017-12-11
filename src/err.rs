use std::ffi;
use std::io;
#[cfg(unix)]
use std::str;

/// Struct that stores a lookup error from `getaddrinfo`
/// or `getnameinfo`. Can automatically be coerced to an io::Error using `?`.
#[derive(Debug)]
pub struct LookupError {
  kind: LookupErrorKind,
  err_num: i32,
  inner: io::Error,
}

impl LookupError {
  /// Match a `gai` error, returning Ok() if it's
  /// `0`. Otherwise return Err(LookupError) with
  /// the specific error details.
  pub fn match_gai_error(err: i32) -> Result<(), Self> {
    match err {
      0 => Ok(()),
      _ => Err(LookupError::new(err)),
    }
  }

  /// Create a new LookupError from a `gai` error,
  /// returned by `getaddrinfo` and `getnameinfo`.
  pub fn new(err: i32) -> Self {
    LookupError {
      kind: LookupErrorKind::new(err),
      err_num: err,
      inner: gai_err_to_io_err(err),
    }
  }
  /// Get the error kind explicitly. If this is an
  /// io::Error, use From/Into to convert it.
  pub fn kind(&self) -> LookupErrorKind {
    self.kind
  }

  /// Get the actual error number. This can be used
  /// to find non-standard return codes from some
  /// implementations (be careful of portability here).
  pub fn error_num(&self) -> i32 {
    self.err_num
  }
}

#[derive(Copy, Clone, Debug)]
pub enum LookupErrorKind {
  /// Temporary failure in name resolution.
  Again,
  /// Invalid value for `ai_flags' field.
  Badflags,
  /// NAME or SERVICE is unknown.
  NoName,
  /// The specified network host exists, but has no data defined.
  NoData,
  /// Non-recoverable failure in name res.
  Fail,
  /// `ai_family' not supported.
  Family,
  /// `ai_socktype' not supported.
  Socktype,
  /// SERVICE not supported for `ai_socktype'.
  Service,
  /// Memory allocation failure.
  Memory,
  /// System error returned in `errno'.
  System,
  /// Either a generic C error, or an unknown result
  /// code.
  IO,
}

impl LookupErrorKind {
  #[cfg(all(not(windows), not(unix)))]
  /// Create a `LookupErrorKind` from a `gai` error.
  fn new(err: i32) -> Self {
    LookupErrorKind::IO
  }

  #[cfg(unix)]
  /// Create a `LookupErrorKind` from a `gai` error.
  fn new(err: i32) -> Self {
    use libc as c;
    match err {
      c::EAI_AGAIN => LookupErrorKind::Again,
      c::EAI_BADFLAGS => LookupErrorKind::Badflags,
      c::EAI_FAIL => LookupErrorKind::Fail,
      c::EAI_FAMILY => LookupErrorKind::Family,
      c::EAI_MEMORY => LookupErrorKind::Memory,
      c::EAI_NONAME => LookupErrorKind::NoName,
      // Not defined in libc?
      // -5 on linux and openbsd
      -5 => LookupErrorKind::NoData,
      c::EAI_SERVICE => LookupErrorKind::Service,
      c::EAI_SOCKTYPE => LookupErrorKind::Socktype,
      c::EAI_SYSTEM => LookupErrorKind::System,
      _ => LookupErrorKind::IO,
    }
  }

  #[cfg(windows)]
  /// Create a `LookupErrorKind` from a `gai` error.
  fn new(err: i32) -> Self {
    use winapi::winerror as e;
    match err as u32 {
      e::WSATRY_AGAIN => LookupErrorKind::Again,
      e::WSAEINTVAL => LookupErrorKind::Badflags,
      e::WSANO_RECOVERY => LookupErrorKind::Fail,
      e::WSAEAFNOSUPPORT => LookupErrorKind::Family,
      e::WSA_NOT_ENOUGH_MEMORY => LookupErrorKind::Memory,
      e::WSAHOST_NOT_FOUND => LookupErrorKind::NoName,
      e::WSANO_DATA => LookupErrorKind::NoData,
      e::WSATYPE_NOT_FOUND => LookupErrorKind::Service,
      e::WSAESOCKTNOSUPPORT => LookupErrorKind::Socktype,
      _ => LookupErrorKind::IO,
    }
  }
}

impl From<LookupError> for io::Error {
  fn from(err: LookupError) -> io::Error {
    err.inner
  }
}

impl From<io::Error> for LookupError {
  fn from(err: io::Error) -> LookupError {
    LookupError {
      kind: LookupErrorKind::IO,
      err_num: 0,
      inner: err,
    }
  }
}

impl From<ffi::NulError> for LookupError {
  fn from(err: ffi::NulError) -> LookupError {
    let err: io::Error = err.into();
    err.into()
  }
}

#[cfg(all(not(windows), not(unix)))]
/// Given a gai error, return an `std::io::Error` with
/// the appropriate error message. Note `0` is not an
/// error, but will still map to an error
pub(crate) fn gai_err_to_io_err(err: i32) -> io::Error {
  match (err) {
    0 => io::Error::new(
      io::ErrorKind::Other,
      "address information lookup success"
    ),
    _ => io::Error::new(
      io::ErrorKind::Other,
      "failed to lookup address information"
    ),
  }
}

#[cfg(unix)]
/// Given a gai error, return an `std::io::Error` with
/// the appropriate error message. Note `0` is not an
/// error, but will still map to an error
pub(crate) fn gai_err_to_io_err(err: i32) -> io::Error {
  use libc::{EAI_SYSTEM, gai_strerror};

  match err {
    0 => return io::Error::new(
      io::ErrorKind::Other,
      "address information lookup success"
    ),
    EAI_SYSTEM => return io::Error::last_os_error(),
    _ => {},
  }

  let detail = unsafe {
    str::from_utf8(ffi::CStr::from_ptr(gai_strerror(err)).to_bytes()).unwrap()
      .to_owned()
  };
  io::Error::new(io::ErrorKind::Other,
    &format!("failed to lookup address information: {}", detail)[..]
  )
}

#[cfg(windows)]
/// Given a gai error, return an `std::io::Error` with
/// the appropriate error message. Note `0` is not an
/// error, but will still map to an error
pub(crate) fn gai_err_to_io_err(err: i32) -> io::Error {
  use ws2_32::WSAGetLastError;
  match err {
    0 => io::Error::new(
      io::ErrorKind::Other,
      "address information lookup success"
    ),
    _ => {
      io::Error::from_raw_os_error(
        unsafe { WSAGetLastError() }
      )
    }
  }
}
