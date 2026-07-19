fn main() {
    #[cfg(target_os = "linux")]
    {
        // skia-safe `gl` feature embeds both GLX and EGL glue that reference
        // core GL / EGL symbols. Link the system libs so mold can resolve them.
        println!("cargo:rustc-link-lib=GL");
        println!("cargo:rustc-link-lib=EGL");
        println!("cargo:rustc-link-lib=X11");
    }
}
