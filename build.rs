use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let pkg_version = env::var("CARGO_PKG_VERSION").unwrap();
    let pkg_desc = env::var("CARGO_PKG_DESCRIPTION").unwrap_or_else(|_| "Floating TODO".to_string());
    let pkg_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "floating-todo".to_string());

    let parts: Vec<&str> = pkg_version.split('.').collect();
    let major = parts.first().unwrap_or(&"0");
    let minor = parts.get(1).unwrap_or(&"0");
    let patch = parts.get(2).unwrap_or(&"0");

    let rc_content = format!(
        r#"#pragma code_page(65001)
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Floating TODO"
            VALUE "FileDescription", "{pkg_desc}"
            VALUE "FileVersion", "{pkg_version}.0"
            VALUE "InternalName", "{pkg_name}.exe"
            VALUE "OriginalFilename", "{pkg_name}.exe"
            VALUE "ProductName", "Floating TODO"
            VALUE "ProductVersion", "{pkg_version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    );

    let rc_path = Path::new(&out_dir).join("version.rc");
    let res_path = Path::new(&out_dir).join("version.res");
    fs::write(&rc_path, rc_content).expect("Failed to write version.rc");

    let rc_tools = ["llvm-rc", "llvm-rc-19", "rc.exe", "x86_64-w64-mingw32-windres", "windres"];
    let mut compiled = false;

    for tool in rc_tools {
        let is_windres = tool.contains("windres");
        let status = if is_windres {
            Command::new(tool)
                .arg("-i")
                .arg(&rc_path)
                .arg("-O")
                .arg("coff")
                .arg("-o")
                .arg(&res_path)
                .status()
        } else {
            Command::new(tool)
                .arg(&rc_path)
                .arg("-FO")
                .arg(&res_path)
                .status()
        };

        if let Ok(s) = status {
            if s.success() {
                compiled = true;
                break;
            }
        }
    }

    if compiled {
        println!("cargo:rustc-link-arg={}", res_path.display());
    } else {
        println!("cargo:warning=No resource compiler found. Windows PE version info was not embedded.");
    }
}
