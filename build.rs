const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
    </application>
  </compatibility>
</assembly>
"#;

fn main() {
    // нативный C-модуль (boost.c лежит рядом)
    println!("cargo:rerun-if-changed=boost.c");
    cc::Build::new().file("boost.c").compile("boost_native");

    println!("cargo:rerun-if-changed=icon.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=advapi32");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Bnova");
        res.set("FileDescription", "Bnova - Буст от nova");
        res.set("CompanyName", "nova");
        res.set("OriginalFilename", "bnova.exe");
        res.set_manifest(MANIFEST);
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon/manifest was not embedded: {}", e);
        }
    }
}
