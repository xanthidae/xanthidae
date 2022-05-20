use std::ffi::{c_void, CStr, CString};
use std::fmt::{Display, Formatter};
use std::mem;
use std::mem::MaybeUninit;
use std::os::raw::c_char;
use std::os::raw::c_int;

pub struct SelectedObject {
    pub object_type: String,
    pub object_owner: String,
    pub object_name: String,
    pub sub_object: String,
}

impl SelectedObject {
    pub fn new(
        object_type: &str,
        object_owner: &str,
        object_name: &str,
        sub_object: &str,
    ) -> SelectedObject {
        SelectedObject {
            object_type: object_type.to_string(),
            object_owner: object_owner.to_string(),
            object_name: object_name.to_string(),
            sub_object: sub_object.to_string(),
        }
    }

    unsafe fn from_raw_parts(
        object_type: *const c_char,
        object_owner: *const c_char,
        object_name: *const c_char,
        sub_object: *const c_char,
    ) -> SelectedObject {
        SelectedObject {
            object_type: CStr::from_ptr(object_type).to_string_lossy().to_string(),
            object_owner: CStr::from_ptr(object_owner).to_string_lossy().to_string(),
            object_name: CStr::from_ptr(object_name).to_string_lossy().to_string(),
            sub_object: CStr::from_ptr(sub_object).to_string_lossy().to_string(),
        }
    }
}

impl Display for SelectedObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(object_type: {}, object_owner: {}, object_name: {}, sub_object: {})",
            self.object_type, self.object_owner, self.object_name, self.sub_object
        )
    }
}

pub trait PlsqlDevApi {
    fn sys_version(&self) -> i32 {
        0
    }
    fn sys_root_dir(&self) -> String {
        "".to_string()
    }
    fn ide_connected(&self) -> bool {
        false
    }
    fn ide_get_text(&self) -> String {
        "".to_string()
    }
    fn ide_get_selected_text(&self) -> String {
        "".to_string()
    }
    fn ide_create_popup_item(&self, _id: i32, _index: i32, _name: &str, _object_type: &str) {}
    fn ide_first_selected_object(&self) -> Option<SelectedObject> {
        None
    }
    fn ide_next_selected_object(&self) -> Option<SelectedObject> {
        None
    }
    fn ide_get_object_source(
        &self,
        _object_type: &str,
        _object_owner: &str,
        _object_name: &str,
    ) -> String {
        "".to_string()
    }
    fn ide_debug_log(&self, _message: &str) {}
    fn ide_plugin_setting(&self, _id: i32, _setting: &str, _value: &str) {}
    unsafe fn set_callback_from_address(&mut self, _index: c_int, _address: *mut c_void) {}
}

pub struct NativePlsqlDevApi {
    sys_version: MaybeUninit<extern "C" fn() -> c_int>,
    sys_root_dir: MaybeUninit<extern "C" fn() -> *mut c_char>,
    ide_connected: MaybeUninit<extern "C" fn() -> bool>,
    ide_get_text: MaybeUninit<extern "C" fn() -> *mut c_char>,
    ide_get_selected_text: MaybeUninit<extern "C" fn() -> *mut c_char>,
    ide_create_popup_item: MaybeUninit<
        extern "C" fn(
            id: c_int,
            index: c_int,
            name: *mut c_char,
            object_type: *mut c_char,
        ) -> c_void,
    >,
    ide_first_selected_object: MaybeUninit<
        extern "C" fn(
            object_type: *mut *mut c_char,
            object_owner: *mut *mut c_char,
            object_name: *mut *mut c_char,
            sub_object: *mut *mut c_char,
        ) -> bool,
    >,
    ide_next_selected_object: MaybeUninit<
        extern "C" fn(
            object_type: *mut *mut c_char,
            object_owner: *mut *mut c_char,
            object_name: *mut *mut c_char,
            sub_object: *mut *mut c_char,
        ) -> bool,
    >,
    ide_get_object_source: MaybeUninit<
        extern "C" fn(
            object_type: *const c_char,
            object_owner: *const c_char,
            object_name: *const c_char,
        ) -> *mut c_char,
    >,
    ide_debug_log: MaybeUninit<extern "C" fn(*const c_char) -> c_void>,
    ide_plugin_setting: MaybeUninit<
        extern "C" fn(plugin_id: c_int, setting: *const c_char, value: *const c_char) -> bool,
    >,
}

impl NativePlsqlDevApi {
    pub fn new() -> NativePlsqlDevApi {
        NativePlsqlDevApi {
            sys_version: MaybeUninit::uninit(),
            sys_root_dir: MaybeUninit::uninit(),
            ide_connected: MaybeUninit::uninit(),
            ide_get_text: MaybeUninit::uninit(),
            ide_get_selected_text: MaybeUninit::uninit(),
            ide_create_popup_item: MaybeUninit::uninit(),
            ide_first_selected_object: MaybeUninit::uninit(),
            ide_next_selected_object: MaybeUninit::uninit(),
            ide_get_object_source: MaybeUninit::uninit(),
            ide_debug_log: MaybeUninit::uninit(),
            ide_plugin_setting: MaybeUninit::uninit(),
        }
    }
}

impl PlsqlDevApi for NativePlsqlDevApi {
    fn sys_version(&self) -> i32 {
        let sys_version = unsafe { self.sys_version.assume_init() };
        sys_version()
    }

    fn sys_root_dir(&self) -> String {
        unsafe {
            let sys_root_dir = self.sys_root_dir.assume_init();
            CStr::from_ptr(sys_root_dir()).to_string_lossy().to_string()
        }
    }

    fn ide_connected(&self) -> bool {
        let ide_connected = unsafe { self.ide_connected.assume_init() };
        ide_connected()
    }

    fn ide_get_text(&self) -> String {
        unsafe {
            let ide_get_text = self.ide_get_text.assume_init();
            CStr::from_ptr(ide_get_text()).to_string_lossy().to_string()
        }
    }

    fn ide_get_selected_text(&self) -> String {
        unsafe {
            let ide_get_selected_text = self.ide_get_selected_text.assume_init();
            CStr::from_ptr(ide_get_selected_text())
                .to_string_lossy()
                .to_string()
        }
    }

    fn ide_create_popup_item(&self, id: i32, index: i32, name: &str, object_type: &str) {
        let ide_create_popup_item = unsafe { self.ide_create_popup_item.assume_init() };
        let c_name: CString = CString::new(name).unwrap();
        let c_object_type = CString::new(object_type).unwrap();
        ide_create_popup_item(
            id,
            index,
            c_name.as_ptr() as *mut c_char,
            c_object_type.as_ptr() as *mut c_char,
        );
    }

    fn ide_first_selected_object(&self) -> Option<SelectedObject> {
        unsafe {
            let ide_first_selected_object = self.ide_first_selected_object.assume_init();

            let mut object_type = MaybeUninit::<*mut c_char>::uninit();
            let mut object_owner = MaybeUninit::<*mut c_char>::uninit();
            let mut object_name = MaybeUninit::<*mut c_char>::uninit();
            let mut sub_object = MaybeUninit::<*mut c_char>::uninit();

            if ide_first_selected_object(
                object_type.as_mut_ptr(),
                object_owner.as_mut_ptr(),
                object_name.as_mut_ptr(),
                sub_object.as_mut_ptr(),
            ) {
                Some(SelectedObject::from_raw_parts(
                    object_type.assume_init(),
                    object_owner.assume_init(),
                    object_name.assume_init(),
                    sub_object.assume_init(),
                ))
            } else {
                None
            }
        }
    }

    fn ide_next_selected_object(&self) -> Option<SelectedObject> {
        unsafe {
            let ide_next_selected_object = self.ide_next_selected_object.assume_init();

            let mut object_type = MaybeUninit::<*mut c_char>::uninit();
            let mut object_owner = MaybeUninit::<*mut c_char>::uninit();
            let mut object_name = MaybeUninit::<*mut c_char>::uninit();
            let mut sub_object = MaybeUninit::<*mut c_char>::uninit();

            if ide_next_selected_object(
                object_type.as_mut_ptr(),
                object_owner.as_mut_ptr(),
                object_name.as_mut_ptr(),
                sub_object.as_mut_ptr(),
            ) {
                Some(SelectedObject::from_raw_parts(
                    object_type.assume_init(),
                    object_owner.assume_init(),
                    object_name.assume_init(),
                    sub_object.assume_init(),
                ))
            } else {
                None
            }
        }
    }

    fn ide_get_object_source(
        &self,
        object_type: &str,
        object_owner: &str,
        object_name: &str,
    ) -> String {
        unsafe {
            let ide_get_object_source = self.ide_get_object_source.assume_init();

            let c_object_type = CString::new(object_type).unwrap();
            let c_object_owner = CString::new(object_owner).unwrap();
            let c_object_name = CString::new(object_name).unwrap();

            let object_source = ide_get_object_source(
                c_object_type.as_ptr(),
                c_object_owner.as_ptr(),
                c_object_name.as_ptr(),
            );

            CStr::from_ptr(object_source).to_string_lossy().to_string()
        }
    }

    fn ide_debug_log(&self, message: &str) {
        let ide_debug_log = unsafe { self.ide_debug_log.assume_init() };
        let c_message = CString::new(message).unwrap();
        ide_debug_log(c_message.as_ptr());
    }

    fn ide_plugin_setting(&self, id: i32, setting: &str, value: &str) {
        let ide_plugin_setting = unsafe { self.ide_plugin_setting.assume_init() };
        let c_setting = CString::new(setting).unwrap();
        let c_value = CString::new(value).unwrap();
        ide_plugin_setting(id, c_setting.as_ptr(), c_value.as_ptr());
    }

    unsafe fn set_callback_from_address(&mut self, index: c_int, address: *mut c_void) {
        match index {
            1 => self.sys_version.as_mut_ptr().write(mem::transmute(address)),
            3 => self
                .sys_root_dir
                .as_mut_ptr()
                .write(mem::transmute(address)),
            11 => self
                .ide_connected
                .as_mut_ptr()
                .write(mem::transmute(address)),
            30 => self
                .ide_get_text
                .as_mut_ptr()
                .write(mem::transmute(address)),
            31 => self
                .ide_get_selected_text
                .as_mut_ptr()
                .write(mem::transmute(address)),
            69 => self
                .ide_create_popup_item
                .as_mut_ptr()
                .write(mem::transmute(address)),
            77 => self
                .ide_first_selected_object
                .as_mut_ptr()
                .write(mem::transmute(address)),
            78 => self
                .ide_next_selected_object
                .as_mut_ptr()
                .write(mem::transmute(address)),
            79 => self
                .ide_get_object_source
                .as_mut_ptr()
                .write(mem::transmute(address)),
            173 => self
                .ide_debug_log
                .as_mut_ptr()
                .write(mem::transmute(address)),
            219 => self
                .ide_plugin_setting
                .as_mut_ptr()
                .write(mem::transmute(address)),
            _ => (),
        };
    }
}
