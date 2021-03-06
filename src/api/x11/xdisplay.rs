use std::ptr;
use std::fmt;
use std::error::Error;
use std::ffi::CString;

use libc;

use super::ffi;
use api::egl::ffi::egl::Egl;
use api::dlopen;

/// A connection to an X server.
pub struct XConnection {
    pub xlib: ffi::Xlib,
    pub xf86vmode: ffi::Xf86vmode,
    pub xcursor: ffi::Xcursor,
    pub xinput2: ffi::XInput2,
    pub glx: Option<ffi::glx::Glx>,
    pub egl: Option<Egl>,
    pub display: *mut ffi::Display,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

pub type XErrorHandler = Option<unsafe extern fn(*mut ffi::Display, *mut ffi::XErrorEvent) -> libc::c_int>;

impl XConnection {
    pub fn new(error_handler: XErrorHandler) -> Result<XConnection, XNotSupported> {
        // opening the libraries
        let xlib = try!(ffi::Xlib::open());
        let xcursor = try!(ffi::Xcursor::open());
        let xf86vmode = try!(ffi::Xf86vmode::open());
        let xinput2 = try!(ffi::XInput2::open());

        unsafe { (xlib.XInitThreads)() };
        unsafe { (xlib.XSetErrorHandler)(error_handler) };

        // TODO: use something safer than raw "dlopen"
        let glx = {
            let mut libglx = unsafe { dlopen::dlopen(b"libGL.so.1\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            if libglx.is_null() {
                libglx = unsafe { dlopen::dlopen(b"libGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            }

            if libglx.is_null() {
                None
            } else {
                Some(ffi::glx::Glx::load_with(|sym| {
                    let sym = CString::new(sym).unwrap();
                    unsafe { dlopen::dlsym(libglx, sym.as_ptr()) }
                }))
            }
        };

        // TODO: use something safer than raw "dlopen"
        let egl = {
            let mut libegl = unsafe { dlopen::dlopen(b"libEGL.so.1\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            if libegl.is_null() {
                libegl = unsafe { dlopen::dlopen(b"libEGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            }

            if libegl.is_null() {
                None
            } else {
                Some(Egl::load_with(|sym| {
                    let sym = CString::new(sym).unwrap();
                    unsafe { dlopen::dlsym(libegl, sym.as_ptr()) }
                }))
            }
        };

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return Err(XNotSupported::XOpenDisplayFailed);
            }
            display
        };

        Ok(XConnection {
            xlib: xlib,
            xf86vmode: xf86vmode,
            xcursor: xcursor,
            xinput2: xinput2,
            glx: glx,
            egl: egl,
            display: display,
        })
    }
}

impl Drop for XConnection {
    #[inline]
    fn drop(&mut self) {
        unsafe { (self.xlib.XCloseDisplay)(self.display) };
    }
}

/// Error returned if this system doesn't have XLib or can't create an X connection.
#[derive(Clone, Debug)]
pub enum XNotSupported {
    /// Failed to load one or several shared libraries.
    LibraryOpenError(ffi::OpenError),
    /// Connecting to the X server with `XOpenDisplay` failed.
    XOpenDisplayFailed,     // TODO: add better message
}

impl From<ffi::OpenError> for XNotSupported {
    #[inline]
    fn from(err: ffi::OpenError) -> XNotSupported {
        XNotSupported::LibraryOpenError(err)
    }
}

impl Error for XNotSupported {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            XNotSupported::LibraryOpenError(_) => "Failed to load one of xlib's shared libraries",
            XNotSupported::XOpenDisplayFailed => "Failed to open connection to X server",
        }
    }

    #[inline]
    fn cause(&self) -> Option<&Error> {
        match *self {
            XNotSupported::LibraryOpenError(ref err) => Some(err),
            _ => None
        }
    }
}

impl fmt::Display for XNotSupported {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter.write_str(self.description())
    }
}
