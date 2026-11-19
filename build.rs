fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("src/icon.ico");
        res.set("ProductName", "Translate App with AI");
        res.set("FileDescription", "Language Translation App with AI and OCR");
        if let Err(err) = res.compile() {
            println!("cargo:warning=Failed to embed Windows icon resource: {err}");
        }
    }
}
