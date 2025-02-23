use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::os::raw::c_uint;
use std::os::raw::{c_char, c_int};
use std::{mem, ptr};

use winapi::shared::winerror::SUCCEEDED;
use winapi::um::combaseapi::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC,
};
use winapi::um::commdlg::GetSaveFileNameA;
use winapi::um::commdlg::LPOPENFILENAMEA;
use winapi::um::commdlg::OFN_ENABLESIZING;
use winapi::um::commdlg::OFN_HIDEREADONLY;
use winapi::um::commdlg::OFN_NODEREFERENCELINKS;
use winapi::um::commdlg::OFN_NONETWORKBUTTON;
use winapi::um::commdlg::OFN_OVERWRITEPROMPT;
use winapi::um::commdlg::OPENFILENAMEA;
use winapi::um::objbase::COINIT_APARTMENTTHREADED;
use winapi::um::shobjidl::{
    IFileDialog, IFileOpenDialog, FILEOPENDIALOGOPTIONS, FOS_FORCEFILESYSTEM, FOS_FORCESHOWHIDDEN,
    FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
};
use winapi::um::shobjidl_core::{CLSID_FileOpenDialog, IShellItem, SIGDN_FILESYSPATH};
use winapi::um::winnt::PWSTR;
use winapi::um::winuser::MessageBoxA;
use winapi::Interface;

use crate::string_utils::{pwstr_to_cstring, vec_with_nul_to_string};

const FILE_FILTER: &[u8] = b"All Files\0*.*\0\0";
const DEFAULT_EXTENSION: &[u8] = b"sql\0";
const BUFFER_SIZE: usize = 1000;

// TODO: Probably replace with MessageBoxW, but oh boy Task Dialogs look so much nicer,
//  see: https://docs.microsoft.com/en-us/windows/win32/controls/task-dialogs
//  and: https://dzone.com/articles/using-new-taskdialog-winapi
pub fn show_message_box(message: &CStr, caption: &CStr, message_box_type: c_uint) -> c_int {
    unsafe {
        MessageBoxA(
            ptr::null_mut(),
            message.as_ptr(),
            caption.as_ptr(),
            message_box_type,
        )
    }
}

// TODO: Also replace with the more modern IFileDialog from `get_save_folder_name()`
pub fn get_save_file_name() -> Result<String, &'static str> {
    unsafe {
        let mut file_name: Vec<u8> = vec![0; BUFFER_SIZE + 1];
        let mut file_title: Vec<u8> = vec![0; BUFFER_SIZE + 1];
        let size = mem::size_of::<OPENFILENAMEA>() as u32;

        let mut ofn = OPENFILENAMEA {
            lStructSize: size,
            hwndOwner: ptr::null_mut(),
            hInstance: ptr::null_mut(),
            lpstrFilter: FILE_FILTER.as_ptr() as *const c_char,
            lpstrCustomFilter: ptr::null_mut(),
            nMaxCustFilter: 0,
            nFilterIndex: 0,
            lpstrFile: file_name.as_mut_ptr() as *mut c_char,
            nMaxFile: BUFFER_SIZE as u32,
            lpstrFileTitle: file_title.as_mut_ptr() as *mut c_char,
            nMaxFileTitle: BUFFER_SIZE as u32,
            lpstrInitialDir: ptr::null_mut(),
            lpstrTitle: ptr::null_mut(),
            Flags: OFN_ENABLESIZING
                | OFN_HIDEREADONLY
                | OFN_NODEREFERENCELINKS
                | OFN_NONETWORKBUTTON
                | OFN_OVERWRITEPROMPT,
            nFileOffset: 0,
            nFileExtension: 0,
            lpstrDefExt: DEFAULT_EXTENSION.as_ptr() as *const c_char,
            lCustData: 0,
            lpfnHook: None,
            lpTemplateName: ptr::null_mut(),
            pvReserved: ptr::null_mut(),
            dwReserved: 0,
            FlagsEx: 0,
        };

        //        debug!("file_name: {:?}\n", file_name);
        //        debug!("file_title: {:?}\n", file_title);

        match GetSaveFileNameA(&mut ofn as LPOPENFILENAMEA) {
            1 => {
                let file_name_str = vec_with_nul_to_string(&file_title);
                match file_name_str.as_ref() {
                    "" => Err("Empty name"),
                    _ => Ok(file_name_str),
                }
            }
            _ => Err("Cancelled"),
        }
    }
}

// see: https://github.com/pachi/rust_winapi_examples/blob/master/src/bin/04_hulc2env_gui.rs
pub fn get_save_folder_name() -> String {
    unsafe {
        let mut selected_folder = CString::new("").unwrap();
        let mut hr = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);

        if SUCCEEDED(hr) {
            let mut file_open_dialog: MaybeUninit<*mut IFileDialog> = MaybeUninit::uninit();

            hr = CoCreateInstance(
                &CLSID_FileOpenDialog,
                ptr::null_mut(),
                CLSCTX_INPROC,
                &IFileOpenDialog::uuidof(),
                file_open_dialog.as_mut_ptr() as *mut *mut winapi::ctypes::c_void,
            );

            if SUCCEEDED(hr) {
                let mut file_open_options: FILEOPENDIALOGOPTIONS = std::mem::zeroed();
                let file_open_dialog_ptr = file_open_dialog.assume_init();
                if SUCCEEDED((*file_open_dialog_ptr).GetOptions(&mut file_open_options)) {
                    (*file_open_dialog_ptr).SetOptions(
                        file_open_options
                            | FOS_PICKFOLDERS
                            | FOS_FORCESHOWHIDDEN
                            | FOS_PATHMUSTEXIST
                            | FOS_FORCEFILESYSTEM,
                    );
                }
                if SUCCEEDED((*file_open_dialog_ptr).Show(ptr::null_mut())) {
                    let mut shell_item: *mut IShellItem = std::mem::zeroed();
                    if SUCCEEDED((*file_open_dialog_ptr).GetResult(&mut shell_item)) {
                        let mut buffer: PWSTR = std::ptr::null_mut();

                        if SUCCEEDED((*shell_item).GetDisplayName(SIGDN_FILESYSPATH, &mut buffer)) {
                            selected_folder = pwstr_to_cstring(buffer);
                        }
                        CoTaskMemFree(buffer as *mut winapi::ctypes::c_void);
                    }
                    (*shell_item).Release();
                }
                (*file_open_dialog_ptr).Release();
            }
        }
        CoUninitialize();
        selected_folder.to_string_lossy().into_owned()
    }
}
