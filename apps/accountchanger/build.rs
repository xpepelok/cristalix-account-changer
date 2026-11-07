fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "AccountChanger");
        resource.set("FileDescription", "Cristalix AccountChanger");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed icon: {error}");
        }
    }
}
