use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice::from_raw_parts;

use winapi::um::winnt::PWSTR;

// Convert a C string (const char*) into a Rust string
// see https://doc.rust-lang.org/std/ffi/struct.CStr.html
#[allow(dead_code)]
pub fn ptr_to_string(ptr: *const c_char) -> String {
    unsafe {
        return CStr::from_ptr(ptr).to_string_lossy().into_owned();
    }
}

// Simply converts a raw C string into a owned Rust CString
#[allow(dead_code)]
pub fn ptr_to_cstring(ptr: *const c_char) -> CString {
    unsafe { CStr::from_ptr(ptr).to_owned() }
}

// Converts a Vec<u8> buffer reference to an owned Rust String
pub fn vec_with_nul_to_string(bytes: &[u8]) -> String {
    let first_nul_char_pos = bytes
        .iter()
        .position(|&c| c == b'\0')
        .expect("Could not find null character in buffer");

    return CStr::from_bytes_with_nul(&bytes[0..first_nul_char_pos + 1])
        .expect("CStr::from_bytes_with_nul failed")
        .to_string_lossy()
        .into_owned();
}

// Converts a Vec<u8> buffer reference to an owned CString
#[allow(dead_code)]
pub fn vec_with_nul_to_cstring(bytes: &[u8]) -> CString {
    let first_nul_char_pos = bytes
        .iter()
        .position(|&c| c == b'\0')
        .expect("Could not find null character in buffer");

    return CStr::from_bytes_with_nul(&bytes[0..first_nul_char_pos + 1])
        .expect("CStr::from_bytes_with_nul failed")
        .to_owned();
}

// Converts a Windows PWSTR (a wchar*) into a Rust CString
pub fn pwstr_to_cstring(ptr: PWSTR) -> CString {
    unsafe {
        let len = (0_usize..)
            .find(|&n| *ptr.offset(n as isize) == 0)
            .expect("Null terminator not found");

        let array: &[u16] = from_raw_parts(ptr, len);
        let str = String::from_utf16_lossy(array);
        CString::new(str).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::string_utils::*;

    #[test]
    fn pwstr_to_cstr_should_work_for_ascii() {
        let input: Vec<u16> = vec![65, 0]; // 65: ASCII code of 'A', PWSTR is just a synonym for *mut u16
        let got: CString = pwstr_to_cstring(input.as_ptr() as *mut u16);
        assert_eq!(CString::new("A").unwrap(), got);
    }

    #[test]
    fn pwstr_to_cstr_should_work_for_umlauts() {
        let input: Vec<u16> = vec![252, 0]; // U+00FD / 252: Unicode codepoint for 'ü'
        let got: CString = pwstr_to_cstring(input.as_ptr() as *mut u16);
        assert_eq!(CString::new("ü").unwrap(), got);
    }

    #[test]
    fn pwstr_to_cstr_should_work_for_russian() {
        let input: Vec<u16> = vec![1080, 0]; // U+0438 / : Unicode codepoint for и (as in Россия (Russia), see https://stackoverflow.com/a/10569477/610979 )
        let got: CString = pwstr_to_cstring(input.as_ptr() as *mut u16);
        assert_eq!(CString::new("и").unwrap(), got);
    }
}
