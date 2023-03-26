use std::env;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_void;
use std::path::PathBuf;
use std::sync::{RwLock, RwLockReadGuard};

use log::LevelFilter;
use simplelog::Config as LogConfig;
use simplelog::WriteLogger;
use winapi::um::winuser::MB_ICONINFORMATION;
use winapi::um::winuser::MB_OK;
use windows::core::PCWSTR;

use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt; // for converting between OsString and Windows-native string types

use crate::config::Config;
use crate::flyway::create_repeatable_migration;
use crate::flyway::create_versioned_migration;
use crate::plsqldev_api::{NativePlsqlDevApi, PlsqlDevApi};
use crate::windows_api::{show_message_box, show_task_dialog};

const PLUGIN_NAME: &[u8] = b"Xanthidae\0";
const TAB_NAME: &[u8] = b"TAB=Xanthidae\0";
const FLYWAY_GROUP_NAME: &[u8] = b"GROUP=Flyway\0";
const ITEM_NAME_VERSIONED_MIGRATION: &[u8] = b"ITEM=Versioned migration\0";
const ITEM_NAME_REPEATABLE_MIGRATION: &[u8] = b"ITEM=Repeatable migration\0";
const ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION: &[u8] =
    b"ITEM=Repeatable + versioned migration\0";
const ITEM_NAME_VERSION_INFO: &[u8] = b"ITEM=Plugin version\0";
const EMPTY: &[u8] = b"\0";

const FUNCTION_OBJECT_TYPE: &str = "FUNCTION";
const PROCEDURE_OBJECT_TYPE: &str = "PROCEDURE";
const PACKAGE_OBJECT_TYPE: &str = "PACKAGE";
const TYPE_OBJECT_TYPE: &str = "TYPE";
const VIEW_OBJECT_TYPE: &str = "VIEW";
const TRIGGER_OBJECT_TYPE: &str = "TRIGGER";

/*const FUNCTIONS_OBJECT_TYPE: &'static [u8] = b"FUNCTION+\0";
const PROCEDURES_OBJECT_TYPE: &'static [u8] = b"PROCEDURE+\0";
const PACKAGES_OBJECT_TYPE: &'static [u8] = b"PACKAGE+\0";
const TYPES_OBJECT_TYPE: &'static [u8] = b"TYPE+\0";
const VIEWS_OBJECT_TYPE: &'static [u8] = b"VIEW+\0";
const TRIGGERS_OBJECT_TYPE: &'static [u8] = b"FUNCTION+\0";*/

const SQL_WINDOW: &str = "SQLWINDOW";
const TEST_WINDOW: &str = "TESTWINDOW";
const COMMAND_WINDOW: &str = "COMMANDWINDOW";

const VERSIONED_MIGRATION_INDEX: c_int = 11;
const REPEATABLE_MIGRATION_INDEX: c_int = 12;
const REPEATABLE_AND_VERSIONED_MIGRATION_INDEX: c_int = 13;
const VERSION_INFO_INDEX: c_int = 14;

const POPUP_ITEM_NAME_VERSIONED_MIGRATION: &str = "Versioned migration...";
const POPUP_ITEM_NAME_REPEATABLE_MIGRATION: &str = "Repeatable migration...";
const POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION: &str =
    "Repeatable + versioned migration...";

const VERSION_INFO_CAPTION: &[u8] = b"Version info\0";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
const VERGEN_GIT_SHA: &str = env!("VERGEN_GIT_SHA");
const HOMEPAGE: &str = "https://github.com/xanthidae/xanthidae";

static mut PLUGIN_ID: c_int = 0;

lazy_static! {
    // Trait object style global wrapper around PL/SQL-Developer API
    // We need to specify this Send type bound, otherwise the code would not compile
    // See https://stackoverflow.com/questions/59679968/static-array-of-trait-objects
    pub static ref API: RwLock<Box<dyn PlsqlDevApi + Send + Sync>> = RwLock::new(Box::new(NativePlsqlDevApi::new()));
    pub static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
    static ref VERSION_MESSAGE: String = format!(
        "This is version {} of Xanthidae, a plugin written in Rust.\n\
        \n\
         Build date: {}\n\
         Git SHA: {}\n\n\
         Homepage: {}",
        VERSION, BUILD_TIMESTAMP, VERGEN_GIT_SHA, HOMEPAGE
    );
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn IdentifyPlugIn(ID: c_int) -> *mut c_char {
    unsafe {
        PLUGIN_ID = ID;
    }
    PLUGIN_NAME.as_ptr() as *mut c_char
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn CreateMenuItem(Index: c_int) -> *mut c_char {
    let result = match Index {
        1 => TAB_NAME.as_ptr(),
        10 => FLYWAY_GROUP_NAME.as_ptr(),
        VERSIONED_MIGRATION_INDEX => ITEM_NAME_VERSIONED_MIGRATION.as_ptr(),
        REPEATABLE_MIGRATION_INDEX => ITEM_NAME_REPEATABLE_MIGRATION.as_ptr(),
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX => {
            ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION.as_ptr()
        }
        VERSION_INFO_INDEX => ITEM_NAME_VERSION_INFO.as_ptr(),
        _ => EMPTY.as_ptr(),
    };
    result as *mut c_char
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn OnMenuClick(Index: c_int) {
    let api = API.read().unwrap();
    let config = CONFIG.read().unwrap();
    match Index {
        VERSIONED_MIGRATION_INDEX => create_versioned_migration(&api, &config),
        REPEATABLE_MIGRATION_INDEX => create_repeatable_migration(&api, &config, false),
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX => {
            create_repeatable_migration(&api, &config, true)
        }
        VERSION_INFO_INDEX => show_plugin_version(),
        _ => (),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn About() -> *mut c_char {
    VERSION_MESSAGE.as_ptr() as *mut c_char
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn RegisterCallback(Index: c_int, Addr: *mut c_void) {
    let mut api = API.write().unwrap();
    unsafe { api.set_callback_from_address(Index, Addr) };
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn OnCreate() {
    let user_profile = env::var("USERPROFILE").unwrap();
    let log_file_path: PathBuf = [user_profile, "rustplugin.log".to_string()]
        .iter()
        .collect();
    WriteLogger::init(
        LevelFilter::Debug,
        LogConfig::default(),
        File::create(log_file_path).unwrap(),
    )
    .unwrap();
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn AfterStart() {
    let api = API.read().unwrap();
    let plugin_id = unsafe { PLUGIN_ID };
    create_menu_items(&api, plugin_id);
    set_charmode(&api, plugin_id);
}

fn create_menu_items_for_repeatable_migrations(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    plugin_id: c_int,
) {
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_MIGRATION,
        FUNCTION_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_MIGRATION,
        PROCEDURE_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_MIGRATION,
        PACKAGE_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_MIGRATION,
        TYPE_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_MIGRATION,
        VIEW_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_MIGRATION,
        TRIGGER_OBJECT_TYPE,
    );
}

fn create_menu_items_for_repeatable_and_versioned_migrations(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    plugin_id: c_int,
) {
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION,
        FUNCTION_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION,
        PROCEDURE_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION,
        PACKAGE_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION,
        TYPE_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION,
        VIEW_OBJECT_TYPE,
    );
    api.ide_create_popup_item(
        plugin_id,
        REPEATABLE_AND_VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_REPEATABLE_AND_VERSIONED_MIGRATION,
        TRIGGER_OBJECT_TYPE,
    );
}

fn create_menu_items_for_versioned_migrations(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    plugin_id: c_int,
) {
    api.ide_create_popup_item(
        plugin_id,
        VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_VERSIONED_MIGRATION,
        SQL_WINDOW,
    );
    api.ide_create_popup_item(
        plugin_id,
        VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_VERSIONED_MIGRATION,
        TEST_WINDOW,
    );
    api.ide_create_popup_item(
        plugin_id,
        VERSIONED_MIGRATION_INDEX,
        POPUP_ITEM_NAME_VERSIONED_MIGRATION,
        COMMAND_WINDOW,
    );
}

fn create_menu_items(api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>, plugin_id: c_int) {
    create_menu_items_for_repeatable_migrations(&api, plugin_id);
    create_menu_items_for_versioned_migrations(&api, plugin_id);
    create_menu_items_for_repeatable_and_versioned_migrations(&api, plugin_id);
}

fn set_charmode(api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>, plugin_id: c_int) {
    api.ide_plugin_setting(plugin_id, "CHARMODE", "UTF8");
}

fn show_plugin_version() {
    let caption = CStr::from_bytes_with_nul(VERSION_INFO_CAPTION).unwrap();
    //let s: PWCSTR = PWCSTR::from("x");
    //let t = w!("x");
    //let s: PCWSTR = PCWSTR::from_raw(VERSION_MESSAGE.as_bytes());

    //let my_string = "Hello, world!";
    //let my_pwcstr: PCWSTR = my_string.to_wide_null();

    let my_string: &str = &VERSION_MESSAGE;
    //let my_string = "Hello, world!";
    let wide_string: Vec<u16> = OsString::from(my_string).encode_wide().chain(Some(0)).collect();
    let my_pwcstr: PCWSTR = PCWSTR::from_raw(wide_string.as_ptr());
    //let wide_string: Vec<u16> = OsString::from(my_string).encode_wide().chain(Some(0)).collect();
    //let my_pwcstr: *const u16 = wide_string.as_ptr();
    show_task_dialog(&"About", &VERSION_MESSAGE); // &VERSION_MESSAGE, caption, MB_OK | MB_ICONINFORMATION);
}
