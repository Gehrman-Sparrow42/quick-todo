use std::{env, path::PathBuf, process::Command};
fn main() {
    for file in [
        "resources/app.rc",
        "resources/app.manifest",
        "resources/quick-todo.ico",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let (tool, resource, args) = if target.ends_with("windows-gnu") {
        (
            "windres",
            out.join("app.o"),
            vec!["-Iresources", "resources/app.rc", "-O", "coff", "-o"],
        )
    } else {
        (
            "rc.exe",
            out.join("app.res"),
            vec!["/nologo", "/Iresources", "/fo"],
        )
    };
    let mut command = Command::new(tool);
    command.args(args).arg(&resource);
    if !target.ends_with("windows-gnu") {
        command.arg("resources/app.rc");
    }
    let status = command
        .status()
        .expect("Windows resource compiler is required");
    assert!(status.success(), "Windows resource compiler failed");
    println!("cargo:rustc-link-arg-bin=quick-todo={}", resource.display());
}
