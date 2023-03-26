#[cfg(target_os = "windows")]
extern crate vergen;
extern crate winres;

use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    use vergen::{vergen, Config};
    // Generate the default 'cargo:' instruction output
    let _vers = vergen(Config::default());
    let res = winres::WindowsResource::new();
    //res.set_icon("xanthidae.ico");
    res.compile().unwrap();
    /* 
    // embed_manifest is required for TaskDialog
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Contoso.Sample")).expect("unable to embed manifest file");
    }
    println!("cargo::rerun-if-changed=build.rs");
    */
}

#[cfg(target_os = "linux")]
fn main() {}
