fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/veruxapp.ico");
        res.set("ProductName", "Verux APB");
        res.set("FileDescription", "Verux APB");
        res.set("OriginalFilename", "verux_apb.exe");
        res.set("InternalName", "verux_apb");
        res.compile().expect("failed to compile Windows resources");
    }
}
