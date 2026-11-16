//! Compile presentation resources, without Qt or platform business logic.
fn main() {
    let config = slint_build::CompilerConfiguration::new().with_library_paths(
        std::collections::HashMap::from([(
            "material".into(),
            std::path::PathBuf::from("../../third_party/slint-material/material.slint"),
        )]),
    );
    slint_build::compile_with_config("ui/main.slint", config)
        .unwrap_or_else(|error| panic!("Compile Slint presentation: {error}"));
}
