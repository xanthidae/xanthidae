pub struct Config {
    pub use_millisecond_precision: bool,
}

impl Config {
    pub fn new(use_millisecond_precision: bool) -> Config {
        Config {
            use_millisecond_precision,
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config::new(false)
    }
}
