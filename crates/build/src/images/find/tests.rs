use super::*;

fn named(kind: ImageKind, file: &str) -> Result<Source, String> {
    source(kind, PathBuf::from(file), &|p| p.display().to_string()).map_err(|e| e.body().remedy)
}

#[test]
fn a_name_becomes_pascal_labels_and_upper_constants() {
    let s = named(ImageKind::Bg, "big-tree_2.png").unwrap();
    assert_eq!(
        (s.label.as_str(), s.constant.as_str()),
        ("BigTree2", "BIG_TREE_2")
    );
    assert_eq!(s.frame, None);
}

#[test]
fn a_sheet_name_may_carry_its_frame_size_and_a_background_name_may_not() {
    let s = named(ImageKind::Sprites, "hero.16x24.png").unwrap();
    assert_eq!((s.label.as_str(), s.frame), ("Hero", Some((16, 24))));
    let remedy = named(ImageKind::Sprites, "hero.big.png").unwrap_err();
    assert!(remedy.contains("hero.16x16.png"), "{remedy}");
    let remedy = named(ImageKind::Sprites, "2 hero.big.png").unwrap_err();
    assert!(remedy.contains(" hero.16x16.png"), "{remedy}");
    let remedy = named(ImageKind::Bg, "town.16x16.png").unwrap_err();
    assert!(remedy.contains("town_16x16.png"), "{remedy}");
}

/// A project directory with these (empty) files under `assets/`.
fn project(name: &str, files: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gbcli-find-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for f in files {
        let path = dir.join("assets").join(f);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }
    dir
}

fn found(dir: &Path) -> Result<Vec<Source>, BuildError> {
    sources(dir, &|p| {
        p.strip_prefix(dir).unwrap_or(p).display().to_string()
    })
}

#[test]
fn a_dmg_png_is_its_color_images_dmg_version_not_an_image() {
    let dir = project(
        "dmg-version",
        &[
            "bg/town.png",
            "bg/town.dmg.png",
            "bg/sky.png",
            "sprites/hero.16x16.png",
            "sprites/hero.16x16.DMG.PNG",
        ],
    );
    let found = found(&dir).unwrap();
    let named: Vec<(&str, Option<PathBuf>)> = found
        .iter()
        .map(|s| {
            let dmg = s.dmg.as_ref().map(|p| p.strip_prefix(&dir).unwrap().into());
            (s.stem.as_str(), dmg)
        })
        .collect();
    assert_eq!(
        named,
        [
            ("sky", None),
            ("town", Some(PathBuf::from("assets/bg/town.dmg.png"))),
            (
                "hero",
                Some(PathBuf::from("assets/sprites/hero.16x16.DMG.PNG"))
            ),
        ]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_dmg_png_without_its_color_image_is_a_layout_error() {
    let dir = project("dmg-orphan", &["bg/town.dmg.png", "bg/sky.png"]);
    let body = found(&dir).unwrap_err().body();
    assert!(
        body.message.contains("assets/bg/town.dmg.png"),
        "{}",
        body.message
    );
    assert!(
        body.message.contains("assets/bg/town.png"),
        "{}",
        body.message
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_name_that_cant_be_a_label_is_offered_one_that_can() {
    assert!(
        named(ImageKind::Bg, "2 town.png")
            .unwrap_err()
            .contains("town.png")
    );
    assert!(
        named(ImageKind::Bg, "café.png")
            .unwrap_err()
            .contains("caf_.png")
    );
}
