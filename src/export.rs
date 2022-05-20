use std::ffi::CStr;
//use std::fs::File;
//use std::os::raw::{c_char, c_ushort};
use std::os::raw::c_char;
use std::sync::RwLock;
//use std::ffi::OsString;
//use std::os::windows::prelude::*;

//use std::os::raw::c_int;
//use std::os::raw::c_void;
use winapi::um::winuser::MB_ICONINFORMATION;
use winapi::um::winuser::MB_OK;

use crate::clipboard::copy_to_clipboard;
use crate::windows_api::show_message_box;

const EXPORT_TO_CLIPBOARD_AS_WIKI: &[u8] = b"Export to clipboard in Wiki syntax (Rust)\0";

pub struct ExportData {
    pub headers: Vec<String>,
    pub data: Vec<Vec<String>>,
    pub current_row: Vec<String>,
    pub prepared: bool,
}

impl ExportData {
    pub fn new() -> ExportData {
        ExportData {
            headers: vec![],
            data: vec![],
            current_row: vec![],
            prepared: false,
        }
    }

    pub fn init(self: &mut ExportData) {
        self.headers = vec![];
        self.data = vec![];
        self.current_row = vec![];
        self.prepared = false;
    }

    pub fn num_columns(self: &ExportData) -> usize {
        return self.headers.len();
    }

    /// convert to string (in Wiki syntax).
    pub fn to_string(self: &ExportData) -> String {
        // TODO: rewrite this in a more functional style, something like headers.join() + data.join() or map or ...
        let mut result: String = String::new();
        result = result + "||";
        for h in &self.headers {
            result = result + &h + "||";
        }
        result = result + "\n";
        for d in &self.data {
            result = result + "|";
            for cell in d {
                result = result + cell + "|";
            }
            result = result + "\n";
        }
        return result;
    }
}

lazy_static! {
  // See https://stackoverflow.com/questions/59679968/static-array-of-trait-objects
  pub static ref EXPORT_DATA: RwLock<ExportData> = RwLock::new(ExportData::new());
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn ExportInit() -> bool {
    //let caption = CStr::from_bytes_with_nul(b"ExportInit\0").unwrap();
    //show_message_box(&caption, &caption, MB_OK | MB_ICONINFORMATION);
    let mut export_data = EXPORT_DATA.write().unwrap();
    export_data.init();
    return true;
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn ExportFinished() {
    //let caption = CStr::from_bytes_with_nul(b"ExportFinished\0").unwrap();
    //show_message_box(&caption, &caption, MB_OK | MB_ICONINFORMATION);
    let export_data = EXPORT_DATA.read().unwrap();
    let res = copy_to_clipboard(&export_data.to_string());
    let caption = match res {
        Ok(_) => CStr::from_bytes_with_nul(b"Results copied to clipboard\0"),
        Err(_e) => CStr::from_bytes_with_nul(
            b"An error occured. If this problem persists, please file a bug report.\0",
        ),
    }
    .unwrap();
    show_message_box(&caption, &caption, MB_OK | MB_ICONINFORMATION);
}

/// One cell of data, this can be the column description or the actual data.
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn ExportData(value: *const c_char) -> bool {
    //let caption = CStr::from_bytes_with_nul(b"ExportData\0").unwrap();
    //show_message_box(&caption, &caption, MB_OK | MB_ICONINFORMATION);
    let mut export_data = EXPORT_DATA.write().unwrap();
    // from https://doc.rust-lang.org/std/os/windows/ffi/index.html - this might work with some tweaking, but currently results in an access violation
    //pub extern "C" fn ExportData(value: &[u16]) -> bool {
    //let string = OsString::from_wide(value);
    /*let str_buf: String = match string.into_string() {
      Ok(s) => s,
      Err(e) => "?".to_string()
    };*/

    let c_str: &CStr = unsafe { CStr::from_ptr(value) };
    // to_str() fails for non UTF-8 input(e.g. Strings containing umlauts - presumably, they're UTF-16 encoded?);
    //   in that case, we simply return a question mark for the whole string
    let str_slice: &str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => "?",
    };
    let str_buf: String = str_slice.to_owned();
    // still in header part? append to header vec
    if !export_data.prepared {
        export_data.headers.push(str_buf);
    }
    // otherwise: append to current row, and start a new row if necessary
    else {
        export_data.current_row.push(str_buf);
        if export_data.current_row.len() == export_data.num_columns() {
            let current_row = export_data.current_row.clone();
            export_data.data.push(current_row);
            export_data.current_row = vec![];
        }
    }
    return true;
}

// This function allows you to prepare for the actual data
// All values received with Exportdata before this function is called are column headers,
// and all values received after ExportPrepare is data.
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn ExportPrepare() -> bool {
    //let caption = CStr::from_bytes_with_nul(b"ExportPrepare\0").unwrap();
    //show_message_box(&caption, &caption, MB_OK | MB_ICONINFORMATION);
    let mut export_data = EXPORT_DATA.write().unwrap();
    export_data.prepared = true;
    return true;
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn RegisterExport() -> *mut c_char {
    return EXPORT_TO_CLIPBOARD_AS_WIKI.as_ptr() as *mut c_char;
}

#[cfg(test)]
mod tests {

    use crate::export::*;

    // Create a vector from string literals, i.e. vec_of_strings!["a", "b", "c"]
    macro_rules! vec_of_strings {
      ($($x:expr),*) => (vec![$($x.to_string()),*]);
    }

    #[test]
    fn to_string_should_return_wiki_syntax() {
        let export_data = ExportData {
            headers: vec_of_strings!["h1", "h2", "h3"],
            data: vec![
                vec_of_strings!["d11", "d12", "d13"],
                vec_of_strings!["d21", "d22", "d23"],
            ],
            current_row: vec![],
            prepared: true,
        };
        assert_eq!(
            "||h1||h2||h3||\n|d11|d12|d13|\n|d21|d22|d23|\n",
            export_data.to_string()
        );
    }
}
