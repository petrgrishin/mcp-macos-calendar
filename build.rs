fn main() {
    println!("cargo:rustc-link-arg=-sectcreate");
    println!("cargo:rustc-link-arg=__TEXT");
    println!("cargo:rustc-link-arg=__info_plist");
    println!("cargo:rustc-link-arg=Info.plist");
}
