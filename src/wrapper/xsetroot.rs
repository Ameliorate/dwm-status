#![allow(unsafe_code)]

use error::*;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use x11::xlib;

pub struct XSetRoot {
    display: *mut xlib::Display,
    root_window: xlib::Window,
}

impl XSetRoot {
    pub fn new() -> Result<Self> {
        unsafe {
            let display = xlib::XOpenDisplay(ptr::null());

            if display.is_null() {
                return Err(Error::new_custom("render", "cannot open display"));
            }

            let screen = xlib::XDefaultScreen(display);
            let root_window = xlib::XRootWindow(display, screen);

            Ok(XSetRoot {
                display,
                root_window,
            })
        }
    }

    pub fn render(&self, text: String) -> Result<()> {
        let status_c = CString::new(text)
            .wrap_error("render", "status text could not be converted to CString")?;

        unsafe {
            xlib::XStoreName(
                self.display,
                self.root_window,
                status_c.as_ptr() as *mut c_char,
            );

            xlib::XFlush(self.display);
        }

        Ok(())
    }
}

impl Drop for XSetRoot {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}
