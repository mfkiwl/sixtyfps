/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::io::Write;
use std::path::PathBuf;

fn os_dylib_prefix_and_suffix() -> (&'static str, &'static str) {
    if cfg!(target_os = "windows") {
        ("", "dll")
    } else if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        ("lib", "dylib")
    } else {
        ("lib", "so")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Variables that cc.rs needs.
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rustc-env=HOST={}", std::env::var("HOST").unwrap());
    println!("cargo:rustc-env=OPT_LEVEL={}", std::env::var("OPT_LEVEL").unwrap());

    // target/{debug|release}/build/package/out/ -> target/{debug|release}
    let mut target_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    target_dir.pop();
    target_dir.pop();
    target_dir.pop();

    println!("cargo:rustc-env=CPP_LIB_PATH={}", target_dir.display());

    let nodejs_native_lib_name = {
        let (prefix, suffix) = os_dylib_prefix_and_suffix();
        format!("{}sixtyfps_node_native.{}", prefix, suffix)
    };
    println!(
        "cargo:rustc-env=SIXTYFPS_NODE_NATIVE_LIB={}",
        target_dir.join(nodejs_native_lib_name).display()
    );

    let mut include_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    include_dir.push("include");
    println!("cargo:rustc-env=GENERATED_CPP_HEADERS_PATH={}", include_dir.display());
    test_driver_lib::cbindgen::gen_all(&include_dir)?;
    // re-run cbindgen if files changes
    let mut manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    manifest_dir.pop();
    manifest_dir.pop();
    println!("cargo:rerun-if-changed={}/sixtyfps_runtime/corelib/", manifest_dir.display());
    for entry in std::fs::read_dir(manifest_dir.join("sixtyfps_runtime/corelib/"))? {
        let entry = entry?;
        if entry.path().extension().map_or(false, |e| e == "rs") {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }

    println!(
        "cargo:rustc-env=CPP_API_HEADERS_PATH={}/api/sixtyfps-cpp/include",
        manifest_dir.display()
    );

    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    let mut test_dirs = std::collections::HashSet::new();

    for testcase in test_driver_lib::collect_test_cases()? {
        test_dirs.insert({
            let mut dir = testcase.absolute_path.clone();
            dir.pop();
            dir
        });

        let test_function_name = testcase
            .relative_path
            .with_extension("")
            .to_string_lossy()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_");

        write!(
            tests_file,
            r##"
            #[test]
            fn test_cpp_{function_name}() {{
                cppdriver::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                }}).unwrap();
            }}

            #[test]
            fn test_interpreter_{function_name}() {{
                interpreter::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                }}).unwrap();
            }}

            #[test]
            fn test_nodejs_{function_name}() {{
                nodejs::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                }}).unwrap();
            }}
        "##,
            function_name = test_function_name,
            absolute_path = testcase.absolute_path.to_string_lossy(),
            relative_path = testcase.relative_path.to_string_lossy(),
        )?;

        let source = std::fs::read_to_string(&testcase.absolute_path)?;
        for path in test_driver_lib::extract_include_paths(&source) {
            let mut abs_path = testcase.absolute_path.clone();
            abs_path.pop();
            abs_path.push(path);
            println!("cargo:rerun-if-changed={}", abs_path.to_string_lossy());
        }
    }

    test_dirs.iter().for_each(|dir| {
        println!("cargo:rerun-if-changed={}", dir.to_string_lossy());
    });

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());

    Ok(())
}
