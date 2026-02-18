fn main() {
    #[cfg(feature = "built")]
    built::write_built_file().expect("Failed to acquire build-time information");
}
