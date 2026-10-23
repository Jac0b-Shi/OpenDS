#[path = "src/language_pack.rs"]
mod language_pack;
fn languages() {
    println!("cargo:rerun-if-changed=locales");
    println!("cargo:rerun-if-changed=src/language_pack.rs");
    let mut paths: Vec<_> = std::fs::read_dir("locales")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ftl"))
        .collect();
    paths.sort();
    let english =
        language_pack::parse(&std::fs::read_to_string("locales/en-US.ftl").unwrap()).unwrap();
    let mut output = String::from("#[allow(non_camel_case_types)]\n#[derive(Clone,Copy,Debug,Eq,PartialEq,Hash)]\npub enum MessageId {\n");
    for key in english.variables.keys() {
        output += &format!("{},\n", key.replace('-', "_"));
    }
    output += "}\nimpl MessageId { pub fn key(self) -> &'static str { match self {\n";
    for key in english.variables.keys() {
        output += &format!("Self::{} => {:?},\n", key.replace('-', "_"), key);
    }
    output += "}}}\npub const BUILTIN: &[(&str, &str)] = &[\n";
    for p in paths {
        let text = std::fs::read_to_string(&p).unwrap();
        let c = language_pack::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(p.file_stem().unwrap().to_str().unwrap(), c.code);
        let count = language_pack::compare(&c, &english).unwrap();
        if ["en-US", "zh-CN"].contains(&c.code.as_str()) {
            assert_eq!(
                count,
                english.variables.len(),
                "Incomplete built-in language: {}",
                c.code
            );
        }
        assert_eq!(
            count,
            c.variables.len(),
            "Unknown message IDs in {}",
            c.code
        );
        output += &format!(
            "({:?}, include_str!({:?})),\n",
            c.code,
            p.canonicalize().unwrap().to_str().unwrap()
        );
    }
    output += "];\n";
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("languages.rs"),
        output,
    )
    .unwrap();
}
fn main() {
    languages();
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
