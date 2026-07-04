fn main() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../src-tauri/icons/icon.ico");
    res.set("ProductName", "HyperType Setup");
    res.set("FileDescription", "HyperType Setup");
    res.set_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>"#,
    );
    if let Err(e) = res.compile() {
        println!("cargo:warning=icon resource not embedded: {e}");
    }
}
