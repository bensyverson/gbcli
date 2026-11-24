# GB Studio batteryless demo

`demo.gb` and `demo.sym` are an unmodified export of GB Studio 4.3.2's default project, built with Settings > Cartridge Type = MBC5 and "Save game data to ROM" checked, then Export ROM (preview and web builds ignore the setting).
`crates/engine/tests/it/flash_save.rs` drives it on the flash model.

GB Studio ([chrismaltby/gb-studio](https://github.com/chrismaltby/gb-studio)) is MIT licensed; its notice is in `LICENSE` beside the ROM, which contains GB Studio's engine and the default project's assets.
To test another export, name it with `GBCLI_GBSTUDIO_BATTERYLESS_ROM` (its `.sym` must sit beside it).
