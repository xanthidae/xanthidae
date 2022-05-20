use scopeguard::defer;
use std::io::Error;
use std::ptr;
use winapi::shared::minwindef::FALSE;
use winapi::um::winbase::{GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use winapi::um::winuser::{CloseClipboard, OpenClipboard, SetClipboardData, CF_UNICODETEXT};

/// copy the given text to the Windows clipboard
/// taken from https://stackoverflow.com/a/62003949/610979
/// TODO: we should probably use the windows crate provided by Microsoft for this instead
pub fn copy_to_clipboard(text: &str) -> Result<(), Error> {
    // Needs to be UTF-16 encoded
    let mut text_utf16: Vec<u16> = text.encode_utf16().collect();
    // And zero-terminated before passing it into `SetClipboardData`
    text_utf16.push(0);
    // Allocate memory
    let hglob =
        unsafe { GlobalAlloc(GMEM_MOVEABLE, text_utf16.len() * std::mem::size_of::<u16>()) };
    if hglob == ptr::null_mut() {
        return Err(Error::last_os_error());
    }
    // Ensure cleanup on scope exit
    defer!(unsafe { GlobalFree(hglob) };);

    // Retrieve writeable pointer to memory
    let dst = unsafe { GlobalLock(hglob) };
    if dst == ptr::null_mut() {
        return Err(Error::last_os_error());
    }
    // Copy data
    unsafe { ptr::copy_nonoverlapping(text_utf16.as_ptr(), dst as _, text_utf16.len()) };
    // Release writeable pointer
    unsafe { GlobalUnlock(hglob) };

    // Everything is set up now, let's open the clipboard
    let success = unsafe { OpenClipboard(ptr::null_mut()) } != FALSE;
    if !success {
        return Err(Error::last_os_error());
    }
    // Ensure cleanup on scope exit
    defer!(unsafe { CloseClipboard() };);
    // And apply data
    let success = unsafe { SetClipboardData(CF_UNICODETEXT, hglob) } != ptr::null_mut();
    if !success {
        return Err(Error::last_os_error());
    }

    Ok(())
}
