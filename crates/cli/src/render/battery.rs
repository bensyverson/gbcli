//! Rows for `power-cycle` and `sav`.

use gbcli_protocol::ResultBody;

use super::Row;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(p) = &r.power_cycle {
        let json = serde_json::to_value(p).expect("serializes");
        out += &Row::new("power_cycle")
            .field("battery", json["battery"].as_str().unwrap_or("?"))
            .opt("bytes", p.bytes)
            .field("clock", json["clock"].as_str().unwrap_or("?"))
            .field("rtc_advanced_s", p.rtc_advanced_s)
            .flag("rom_rebuilt", p.rom_rebuilt)
            .line();
    }
    if let Some(s) = &r.sav {
        out += &Row::new("sav")
            .field("path", &s.path)
            .field("bytes", s.bytes)
            .field("ram_bytes", s.ram_bytes)
            .line();
    }
    out
}
