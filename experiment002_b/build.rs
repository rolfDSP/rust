fn main() {
    // Compile the .slint UI into Rust code at build time.
    // "fluent-dark" gives the standard widgets (buttons, ...) a dark look
    // that matches the dark blue window backgrounds.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint build failed");
}
