#[cfg(target_os = "windows")]
extern crate vergen;
extern crate winres;

fn main() {
    use vergen::{vergen, Config};
    // Generate the default 'cargo:' instruction output
    let _vers = vergen(Config::default());
    let res = winres::WindowsResource::new();
    //res.set_icon("xanthidae.ico");
    res.compile().unwrap();
}

#[cfg(target_os = "linux")]
fn main() {}
