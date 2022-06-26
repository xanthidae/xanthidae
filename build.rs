#[cfg(target_os="windows")]
extern crate winres;
#[cfg(target_os="windows")]
extern crate vergen;

#[cfg(target_os="windows")]
fn main() {
    use vergen::{Config, vergen};
    // Generate the default 'cargo:' instruction output
    let _vers = vergen(Config::default());
    let res = winres::WindowsResource::new();
    //res.set_icon("xanthidae.ico");
    res.compile().unwrap();
}
#[cfg(target_os="linux")]
fn main() {}
