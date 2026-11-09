//! The native side of `node/colors.mjs`: `native_colors ROM` prints
//! `gbcli_web::color_table_sha256` of ROM.

fn main() {
    let rom = std::env::args().nth(1).expect("usage: native_colors ROM");
    let bytes = std::fs::read(&rom).expect("read ROM");
    println!(
        "{}",
        gbcli_web::color_table_sha256(&bytes).expect("color table")
    );
}
