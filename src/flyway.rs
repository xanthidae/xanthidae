use std::ffi::{CStr, CString};
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::RwLockReadGuard;

use chrono::Utc;
use indoc::indoc;
use regex::{Captures, Regex, RegexBuilder};
use winapi::um::winuser::{MB_ICONERROR, MB_ICONINFORMATION, MB_OK};

use crate::config::Config;
use crate::plsqldev_api::{PlsqlDevApi, SelectedObject};
use crate::windows_api::{get_save_file_name, get_save_folder_name, show_message_box};

const COWARDLY_REFUSING_TO_CREATE_EMPTY_MIGRATION: &str = indoc! { "
  Cowardly refusing to create an empty migration.
  Please select some text and try again.
  "};

const EMPTY_FILE_NAME: &str = "Please enter a file name!";

#[derive(Debug)]
enum FlywayError {
    EmptySelectionError,
    EmptyFileName,
    IOError(String),
}

impl Display for FlywayError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            FlywayError::EmptySelectionError => {
                COWARDLY_REFUSING_TO_CREATE_EMPTY_MIGRATION.to_string()
            }
            FlywayError::EmptyFileName => EMPTY_FILE_NAME.to_string(),
            FlywayError::IOError(s) => format!("I/O error: {}", s),
        };
        write!(f, "{}", msg)
    }
}

impl From<std::io::Error> for FlywayError {
    fn from(e: std::io::Error) -> FlywayError {
        FlywayError::IOError(format!("{}", e))
    }
}

// Create a versioned migration for Flyway
//
// Extracts the currently selected text, asks user for base filename, and writes the
// text to a file whose name is automatically generated as V<timestamp>__<basename>.sql
pub fn create_versioned_migration(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    config: &Config,
) {
    let result = create_versioned_migration_impl(&api, config, get_save_file_name);

    if let Err(e) = result {
        let caption = CString::new("Error").unwrap();
        let message = CString::new(format!("{}", e)).unwrap();
        show_message_box(&message, &caption, MB_OK | MB_ICONERROR);
    }
}

fn create_versioned_migration_impl(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    config: &Config,
    get_save_file_name: fn() -> Result<String, &'static str>,
) -> std::result::Result<(), FlywayError> {
    let ddl = api.ide_get_selected_text();
    // bail out if current selection is empty
    if ddl.len() == 0 {
        return Err(FlywayError::EmptySelectionError);
    }
    // get basename from user, and construct versioned file name
    let basename = get_save_file_name();

    if let Err(message) = basename {
        return match message {
            "Cancelled" => Ok({}),
            "Empty name" => Err(FlywayError::EmptyFileName),
            _ => Err(FlywayError::IOError(message.to_string())),
        };
    }

    let filename = get_versioned_filename(config, &basename.unwrap());
    // write DDL to output file
    let file = File::create(filename);
    let res = match file {
        Ok(mut f) => f.write_all(ddl.as_bytes()),
        Err(e) => Err(e),
    };
    // convert from Result<(), std::io::Error> to Result<(), FlywayError>
    return res.map_err(|e| FlywayError::IOError(format!("{}", e)));
}

fn get_versioned_filename(config: &Config, basename: &str) -> String {
    let now = Utc::now();
    get_versioned_filename_impl(config, now, basename)
}

fn get_versioned_filename_impl(
    config: &Config,
    timestamp: chrono::DateTime<chrono::Utc>,
    basename: &str,
) -> String {
    // construct filename: V<timestamp>_<basename>.sql
    // if basename already contains a .sql suffix, it is removed so we don't get filenams with suffix .sql.sql
    // the user can opt in to include milliseconds in the timestamp to avoid collisions if two developers create migrations
    // at the exact same second
    // CAUTION: only 3f and 6f are supported - trying to use eg 2f causes an External Exception /
    //          thread 'main' panicked at 'a Display implementation return an error unexpectedly: Error'
    //          at runtime!
    let version = match config.use_millisecond_precision {
        true => timestamp.format("V%Y_%m_%d_%H_%M_%S%.3f__"),
        false => timestamp.format("V%Y_%m_%d_%H_%M_%S__"),
    };
    let result = format!("{}{}.sql", version, basename.trim_end_matches(".sql"));
    result
}

const NO_OBJECT_SELECTED_MESSAGE: &[u8] = b"Please select an object in the object browser first!\0";
const NO_OBJECT_SELECTED_CAPTION: &[u8] = b"Nothing selected\0";

pub fn create_repeatable_migration(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    config: &Config,
    export_versioned: bool,
) {
    if let Some(selected_object) = api.ide_first_selected_object() {
        // ME 2021-07-18: #48, do not support multi-export with versioned migration
        if export_versioned && api.ide_next_selected_object().is_some() {
            let message = CString::new("Exporting multiple selected objects as versioned and repeatable migrations is not supported!").unwrap();
            let caption = CString::new("Information").unwrap();
            show_message_box(&message, &caption, MB_OK | MB_ICONINFORMATION);
            return;
        }

        debug!("Selected object: {}", selected_object);

        let folder_name = get_save_folder_name();
        debug!("Selected folder: {:?}", folder_name);

        let mut objects_exported = 0;

        if export_object_as_repeatable_migration(
            &api,
            &folder_name,
            &selected_object,
            config,
            export_versioned,
        )
        .is_ok()
        {
            objects_exported += 1
        }

        while let Some(selected_object) = api.ide_next_selected_object() {
            debug!("Selected object: {}", selected_object);

            if export_object_as_repeatable_migration(
                &api,
                &folder_name,
                &selected_object,
                config,
                export_versioned,
            )
            .is_ok()
            {
                objects_exported += 1
            }
        }

        let caption = CString::new("Repeatable migration").unwrap();
        if objects_exported > 0 {
            let message = CString::new(format!(
                "Successfully exported {} objects as repeatable migration(s).",
                objects_exported
            ))
            .unwrap();
            show_message_box(&message, &caption, MB_OK | MB_ICONINFORMATION);
        } else {
            let message = CString::new("No repeatable migrations were created!\nPlease make sure you have selected one or more supported\nobject types.").unwrap();
            show_message_box(&message, &caption, MB_OK | MB_ICONERROR);
        }
    } else {
        let message = CStr::from_bytes_with_nul(NO_OBJECT_SELECTED_MESSAGE).unwrap();
        let caption = CStr::from_bytes_with_nul(NO_OBJECT_SELECTED_CAPTION).unwrap();
        show_message_box(message, caption, MB_OK | MB_ICONINFORMATION);
    }
}

const SUPPORTED_OBJECT_TYPES: [&str; 6] = [
    "FUNCTION",
    "PROCEDURE",
    "PACKAGE",
    "TYPE",
    "VIEW",
    "TRIGGER",
];

// not sure we actually need the sub_object from above
fn export_object_as_repeatable_migration(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    folder_name: &str,
    selected_object: &SelectedObject,
    config: &Config,
    export_versioned: bool,
) -> std::io::Result<()> {
    // check for supported object type
    if !SUPPORTED_OBJECT_TYPES.contains(&selected_object.object_type.as_str()) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "{} is not a supported object type",
                selected_object.object_type
            ),
        ));
    }

    let object_source = match selected_object.object_type.as_str() {
        "PACKAGE" | "TYPE" => get_object_source_and_body(api, selected_object),
        _ => get_object_source(api, selected_object),
    };

    let basename = selected_object.object_name.to_uppercase();
    if export_versioned {
        let versioned_file_name = get_versioned_filename(config, &basename);
        let path: PathBuf = [folder_name, &versioned_file_name].iter().collect();
        // TODO I don't like the _ assignment - perhaps there's a more elegant way using and_then / map or similar?
        let _ = match File::create(path) {
            Ok(mut f) => f.write_all(object_source.as_bytes()),
            Err(e) => return Err(e),
        };
    }
    let file_name = format!("R__{}.sql", basename);
    let path: PathBuf = [folder_name, &file_name].iter().collect();
    return match File::create(path) {
        Ok(mut f) => f.write_all(object_source.as_bytes()),
        Err(e) => Err(e),
    };
}

// fetches the source of a package or type including its body
fn get_object_source_and_body(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    selected_object: &SelectedObject,
) -> String {
    lazy_static! {
        static ref OBJECT_BODY_NOT_AVAILABLE: Regex = Regex::new(
            r#"/\* Source of (TYPE|PACKAGE) BODY [A-Za-z0-9$_"]+ is not available \*/.*"#
        )
        .unwrap();
    }

    let object_spec = api.ide_get_object_source(
        &selected_object.object_type,
        &selected_object.object_owner,
        &selected_object.object_name,
    );

    let object_spec_incl_owner = ensure_owner_in_ddl(
        &object_spec,
        &selected_object.object_type,
        &selected_object.object_owner,
        &selected_object.object_name,
    );

    let type_of_object_body = match selected_object.object_type.as_str() {
        "PACKAGE" => "PACKAGE BODY",
        "TYPE" => "TYPE BODY",
        _ => "",
    };

    let object_body = api.ide_get_object_source(
        type_of_object_body,
        &selected_object.object_owner,
        &selected_object.object_name,
    );

    let object_body_incl_owner = ensure_owner_in_ddl(
        &object_body,
        type_of_object_body,
        &selected_object.object_owner,
        &selected_object.object_name,
    );

    return match OBJECT_BODY_NOT_AVAILABLE.is_match(&object_body_incl_owner.trim()) {
        true => format!("{}\n/\n", object_spec_incl_owner.trim()),
        _ => format!(
            "{}\n/\n{}\n/\n",
            object_spec_incl_owner.trim(),
            object_body_incl_owner.trim()
        ),
    };
}

// fetches the object source of views, triggers, functions and procedures
fn get_object_source(
    api: &RwLockReadGuard<Box<dyn PlsqlDevApi + Send + Sync>>,
    selected_object: &SelectedObject,
) -> String {
    let object_source = api.ide_get_object_source(
        &selected_object.object_type,
        &selected_object.object_owner,
        &selected_object.object_name,
    );

    // TODO: append "/\n" at the end of functions and procedures
    ensure_owner_in_ddl(
        &object_source,
        &selected_object.object_type,
        &selected_object.object_owner,
        &selected_object.object_name,
    )
}

// Replace the type name in the DDL with owner.type, and optionally enforce creation of the object type
fn ensure_owner_in_ddl(
    ddl: &str,
    object_type: &str,
    object_owner: &str,
    object_name: &str,
) -> String {
    lazy_static! {
        static ref DDL: Regex = RegexBuilder::new(r#"create or replace (editionable|noneditionable)?\s*(package|type|view|trigger|function|procedure)\s*(body )?[a-z0-9_$"]+\s*(\([a-z0-9._$", ]+\))?\s*(force )?(is|as)?(.*)"#)
                            .case_insensitive(true)
                            .build()
                            .unwrap();
    }

    debug!("Object source: {}", ddl);

    // It's necessary to replace $ with $$ as it's used by the Regex crate for capture group references
    // Update 2021-04-02: Seems no longer necessary for whatever reasons, maybe because of the lambda
    let result = DDL.replace(ddl, |caps: &Captures| {
        format!("create or replace {editionable}{force_view}{object_type} {body}{object_owner}.{object_name}{parameter_list}{force_type}{is_or_as}{rest_of_line}",
                editionable = match (caps.get(1).map_or("", |m| m.as_str())).to_lowercase().as_str() {
                    "editionable" => "editionable ",
                    "noneditionable" => "noneditionable ",
                    _ => ""
                },
                force_view = match object_type {
                    "VIEW" => "force ",
                    _ => ""
                },
                object_type = (caps.get(2).map_or("", |m| m.as_str())).to_lowercase(),
                body = (caps.get(3).map_or("", |m| m.as_str())).to_lowercase(),
                object_owner = object_owner,
                object_name = object_name,
                parameter_list = format!("{} ", caps.get(4).map_or("", |m| m.as_str())),
                force_type = match object_type {
                    "TYPE" => "force ",
                    _ => ""
                },
                is_or_as = match object_type {
                    "TRIGGER" => "\n".to_string(),
                    _ => (caps.get(6).map_or("", |m| m.as_str())).to_lowercase()
                }, // insert a line break for triggers
                rest_of_line = caps.get(7).map_or("", |m| m.as_str())
        )
    });

    debug!("Final DDL: {}", result);
    result.to_owned().to_string()
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::sync::RwLock;
    use std::{env, fs};

    use chrono::TimeZone;
    // have to re-import here, otherwise I get stupid 'unused imports' warnings during `cargo build`
    use indoc::indoc;

    use crate::config::Config;
    use crate::flyway::{create_versioned_migration_impl, get_versioned_filename_impl};
    use crate::plsqldev_api::{PlsqlDevApi, SelectedObject};

    use super::export_object_as_repeatable_migration;

    lazy_static! {
        static ref TMP_DIR: String = env::var("TMP").unwrap();
    }

    const PACKAGE_SPEC: &str = indoc! { "
    create or replace noneditionable package pkg_noneditionable is

    end pkg_noneditionable;
    " };
    const PACKAGE_BODY: &str = indoc! { "\
    create or replace noneditionable package body pkg_noneditionable is

    end pkg_noneditionable;
    " };

    const VIEW: &str = indoc! { r#"
    create or replace view v_all_objects as
    select ao."OWNER",
           ao."OBJECT_NAME",
           ao."SUBOBJECT_NAME",
           ao."OBJECT_ID",
           ao."DATA_OBJECT_ID",
           ao."OBJECT_TYPE",
           ao."CREATED",
           ao."LAST_DDL_TIME",
           ao."TIMESTAMP",
           ao."STATUS",
           ao."TEMPORARY",
           ao."GENERATED",
           ao."SECONDARY",
           ao."NAMESPACE",
           ao."EDITION_NAME",
           ao."SHARING",
           ao."EDITIONABLE",
           ao."ORACLE_MAINTAINED",
           ao."APPLICATION",
           ao."DEFAULT_COLLATION",
           ao."DUPLICATED",
           ao."SHARDED",
           ao."CREATED_APPID",
           ao."CREATED_VSNID",
           ao."MODIFIED_APPID",
           ao."MODIFIED_VSNID"
      from all_objects ao;
    "# };

    const PACKAGE_SPEC_WITH_UNICODE_CHARACTERS: &str = indoc! { r#"
    create or replace package DEMO_USER.PKG_SNAFU is
      CHARS constant varchar2(9 byte) := '€µψΨ';
    end pkg_snafu;
    /
    "# };

    struct MockPlsqlDevApi {
        test_type: String,
    }

    impl MockPlsqlDevApi {
        fn new(test_type: &str) -> MockPlsqlDevApi {
            MockPlsqlDevApi {
                test_type: test_type.to_string(),
            }
        }
    }

    impl PlsqlDevApi for MockPlsqlDevApi {
        fn ide_get_selected_text(&self) -> String {
            match self.test_type.as_str() {
                "versioned_migration_with_unicode_characters" => {
                    PACKAGE_SPEC_WITH_UNICODE_CHARACTERS.to_string()
                }
                _ => "".to_string(),
            }
        }

        fn ide_get_object_source(
            &self,
            object_type: &str,
            _object_owner: &str,
            _object_name: &str,
        ) -> String {
            match self.test_type.as_str() {
                "noneditionable_package" => match object_type {
                    "PACKAGE BODY" => PACKAGE_BODY.to_string(),
                    _ => PACKAGE_SPEC.to_string(),
                },
                "view" => VIEW.to_string(),
                _ => "".to_string(),
            }
        }
    }

    fn create_rwlock(test_type: &str) -> RwLock<Box<dyn PlsqlDevApi + Send + Sync>> {
        RwLock::new(Box::new(MockPlsqlDevApi::new(test_type)))
    }

    #[test]
    fn create_repeatable_migration_for_noneditionable_package() {
        let api = create_rwlock("noneditionable_package");
        let guard = api.read().unwrap();
        let selected_object = SelectedObject::new("PACKAGE", "APP", "PKG_NONEDITIONABLE", "");

        if let Err(e) = export_object_as_repeatable_migration(
            &guard,
            &TMP_DIR,
            &selected_object,
            &Config::default(),
            false,
        ) {
            panic!("Exporting object failed, reason: {}", e);
        }

        let output_file: PathBuf = [&TMP_DIR, "R__PKG_NONEDITIONABLE.sql"].iter().collect();

        let expected = indoc! { "
               create or replace noneditionable package APP.PKG_NONEDITIONABLE is

               end pkg_noneditionable;
               /
               create or replace noneditionable package body APP.PKG_NONEDITIONABLE is

               end pkg_noneditionable;
               /
            "};

        assert_eq!(expected, get_contents_of_file(&output_file));
    }

    #[test]
    fn create_repeatable_migration_from_view() {
        let api = create_rwlock("view");
        let guard = api.read().unwrap();
        let selected_object = SelectedObject::new("VIEW", "APP", "V_ALL_OBJECTS", "");

        if let Err(e) = export_object_as_repeatable_migration(
            &guard,
            &TMP_DIR,
            &selected_object,
            &Config::default(),
            false,
        ) {
            panic!("Exporting object failed, reason: {}", e);
        }

        let output_file: PathBuf = [&TMP_DIR, "R__V_ALL_OBJECTS.sql"].iter().collect();

        let expected = indoc! {r#"
             create or replace force view APP.V_ALL_OBJECTS as
             select ao."OWNER",
                    ao."OBJECT_NAME",
                    ao."SUBOBJECT_NAME",
                    ao."OBJECT_ID",
                    ao."DATA_OBJECT_ID",
                    ao."OBJECT_TYPE",
                    ao."CREATED",
                    ao."LAST_DDL_TIME",
                    ao."TIMESTAMP",
                    ao."STATUS",
                    ao."TEMPORARY",
                    ao."GENERATED",
                    ao."SECONDARY",
                    ao."NAMESPACE",
                    ao."EDITION_NAME",
                    ao."SHARING",
                    ao."EDITIONABLE",
                    ao."ORACLE_MAINTAINED",
                    ao."APPLICATION",
                    ao."DEFAULT_COLLATION",
                    ao."DUPLICATED",
                    ao."SHARDED",
                    ao."CREATED_APPID",
                    ao."CREATED_VSNID",
                    ao."MODIFIED_APPID",
                    ao."MODIFIED_VSNID"
               from all_objects ao;
    "# };

        assert_eq!(expected, get_contents_of_file(&output_file));
    }

    #[test]
    fn create_versioned_migration_from_package_with_unicode_characters() {
        const EXPECTED: &str = indoc! { r#"
           create or replace package DEMO_USER.PKG_SNAFU is
             CHARS constant varchar2(9 byte) := '€µψΨ';
           end pkg_snafu;
           /
           "# };

        let api = create_rwlock("versioned_migration_with_unicode_characters");
        let guard = api.read().unwrap();
        let res = create_versioned_migration_impl(&guard, &Config::default(), get_save_file_name);
        assert_eq!(true, res.is_ok());
        // now find the output file
        // search in current directory for now as get_versioned_filename() does not work correctly
        let files = fs::read_dir(&*TMP_DIR).unwrap();
        for file in files.flatten() {
            let file_name = file.file_name().to_string_lossy().into_owned();
            let path = file.path();

            if file_name.contains("PKG_SNAFU") {
                let file_contents = get_contents_of_file(&path);
                assert_eq!(file_contents, EXPECTED);

                if fs::remove_file(&path).is_err() {
                    panic!(
                        "Could not delete versioned migration output file {:?}",
                        &path
                    );
                }
                return;
            }
        }
        panic!("Output file of versioned migration not found!");
    }

    fn get_contents_of_file(output_file: &Path) -> String {
        match File::open(output_file) {
            Ok(mut file) => {
                let mut file_content = String::new();
                file.read_to_string(&mut file_content).unwrap();
                file_content
            }
            Err(e) => panic!(
                "Could not read contents of expected output file, reason: {}",
                e
            ),
        }
    }

    fn get_save_file_name() -> Result<String, &'static str> {
        // TODO instead of relying on the path that SaveFileDialog set as a side effect, we should use the PathBuf approach
        /* let path: PathBuf = [&TMP_DIR, "PKG_SNAFU.sql"].iter().collect();
        return CString::new(path.into_os_string().to_string_lossy().into_owned()).unwrap();*/
        assert!(env::set_current_dir(Path::new(&*TMP_DIR)).is_ok());
        Ok("PKG_SNAFU.sql".to_string())
    }

    struct MockEmptySelectedTextPlsqlDevApi {}

    impl MockEmptySelectedTextPlsqlDevApi {
        fn new() -> MockEmptySelectedTextPlsqlDevApi {
            MockEmptySelectedTextPlsqlDevApi {}
        }
    }

    impl PlsqlDevApi for MockEmptySelectedTextPlsqlDevApi {
        fn ide_get_selected_text(&self) -> String {
            "".to_string()
        }
    }

    fn create_rwlock_mockemptyselectedtext() -> RwLock<Box<dyn PlsqlDevApi + Send + Sync>> {
        RwLock::new(Box::new(MockEmptySelectedTextPlsqlDevApi::new()))
    }

    #[test]
    fn create_versioned_migration_with_empty_selection_should_return_error() {
        let api = create_rwlock_mockemptyselectedtext();
        let guard = api.read().unwrap();
        let res = create_versioned_migration_impl(&guard, &Config::default(), get_save_file_name);
        match res {
            Ok(_) => panic!("This should have returned an error"),
            Err(_) => (),
        }
    }

    #[test]
    fn get_versioned_filename_impl_should_use_provided_timestamp() {
        let timestamp = chrono::Utc.ymd(1970, 1, 2).and_hms(3, 4, 5);
        let basename = "do_it.sql";
        let got = get_versioned_filename_impl(&Config::default(), timestamp, basename);
        assert_eq!("V1970_01_02_03_04_05__do_it.sql", got);
    }

    #[test]
    fn get_versioned_filename_impl_should_add_sql_suffix() {
        let timestamp = chrono::Utc.ymd(1970, 1, 2).and_hms(3, 4, 5);
        let basename = "do_it";
        let got = get_versioned_filename_impl(&Config::default(), timestamp, basename);
        assert_eq!("V1970_01_02_03_04_05__do_it.sql", got);
    }

    #[test]
    fn get_versioned_filename_impl_should_take_config_into_account() {
        let timestamp = chrono::Utc.ymd(1970, 1, 2).and_hms_micro(3, 4, 5, 678000);
        let basename = "do_it";
        let config = Config::new(true);
        let got = get_versioned_filename_impl(&config, timestamp, basename);
        assert_eq!("V1970_01_02_03_04_05.678__do_it.sql", got);
    }
}
