#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
  let version = env!("CARGO_PKG_VERSION");
  let mut res = winres::WindowsResource::new();
  res.set("FileDescription", "A windows management utility for Windows");
  res.set("ProductName", "Randolf");
  res.set("InternalName", "Randolf");
  res.set("OriginalFilename", "randolf.exe");
  res.set("LegalCopyright", "© 2025 Kim Goetzke");
  res.set_icon("assets/randolf.ico");
  res.set_manifest(&format!(
    r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
      version="{version}.0"
      processorArchitecture="*"
      name="kimgoetzke.Randolf"
      type="win32" />
  <description>A window management utility for Windows</description>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#,
  ));
  res.compile().unwrap();
}

#[cfg(not(windows))]
fn main() {}
