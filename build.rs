#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
    let res = winres::WindowsResource::new();
    //res.set_icon("xanthidae.ico");
    res.compile().unwrap();
}

#[cfg(unix)]
fn main() {
}