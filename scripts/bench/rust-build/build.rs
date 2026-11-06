fn main() {
    let mut b = cc::Build::new();
    for e in std::fs::read_dir(&format!("{}/Core", std::env::var("SAMEBOY").expect("set SAMEBOY to a SameBoy checkout"))).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x == "c").unwrap_or(false) { b.file(p); }
    }
    b.include(std::env::var("SAMEBOY").unwrap()).define("GB_INTERNAL", None).define("_GNU_SOURCE", None)
     .define("GB_VERSION", "\"1.0.5\"").define("GB_COPYRIGHT_YEAR", "\"2026\"")
     .flag("-std=gnu11").flag("-Wno-multichar").warnings(false).opt_level(2).compile("sameboy");
}
