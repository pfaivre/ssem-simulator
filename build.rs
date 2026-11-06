fn main() {
    // rust_i18n::i18n! embeds the catalogs at compile time, so cargo must be
    // told that editing locales/ invalidates the build
    println!("cargo:rerun-if-changed=locales");
}
