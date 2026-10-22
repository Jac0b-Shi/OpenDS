fn main() {
    println!("cargo:rerun-if-changed=assets/opends.manifest");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("assets/opends.manifest");
        println!("cargo:rustc-link-arg-bin=opends=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=opends=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo:rerun-if-changed=assets/opends.ico");
        println!("cargo:rerun-if-changed=assets/opends.rc");
        let sdk_bin = std::path::PathBuf::from(
            std::env::var_os("ProgramFiles(x86)")
                .unwrap_or_else(|| "C:/Program Files (x86)".into()),
        )
        .join("Windows Kits/10/bin");
        let mut versions: Vec<_> = std::fs::read_dir(sdk_bin)
            .expect("Windows SDK required")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        versions.sort();
        let rc = std::env::var_os("RC")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                versions
                    .into_iter()
                    .rev()
                    .map(|p| p.join("x64/rc.exe"))
                    .find(|p| p.is_file())
            })
            .expect("Windows SDK rc.exe required");
        let resource =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("opends.res");
        let assets = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("assets");
        let status = std::process::Command::new(rc)
            .current_dir(assets)
            .arg("/nologo")
            .arg("/fo")
            .arg(&resource)
            .arg("opends.rc")
            .status()
            .expect("run resource compiler");
        assert!(status.success(), "compile Windows icon");
        println!("cargo:rustc-link-arg-bin=opends={}", resource.display());
    }
}
