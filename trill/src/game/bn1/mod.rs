use super::{BackgroundRef, Game, LazyImage, SaveTemplates};
use crate::bnlc;
use std::sync::LazyLock;
use trill_dataview::save::Save as SaveTrait;

const MATCH_TYPES: &[usize] = &[1];
const BACKGROUND: BackgroundRef = BackgroundRef {
    volume: bnlc::Volume::Vol1,
    tga: "01.tga",
};

// ---------------- EXE1 JP ----------------
static EXE1_SAVE: LazyLock<trill_dataview::game::bn1::save::Save> = LazyLock::new(|| {
    trill_dataview::game::bn1::save::Save::from_wram(
        include_bytes!("save/jp.raw"),
        trill_dataview::game::bn1::save::GameInfo {
            region: trill_dataview::game::bn1::save::Region::JP,
        },
    )
    .unwrap()
});
static EXE1_T: SaveTemplates = LazyLock::new(|| vec![("", &*EXE1_SAVE as &(dyn SaveTrait + Send + Sync))]);
static EXE1_LOGO: LazyImage =
    LazyLock::new(|| image::load_from_memory(include_bytes!("../../logos/exe1-0.png")).unwrap());

pub static EXE1: Game = Game {
    gamedb_entry: &trill_gamedb::AREJ_00,
    hooks: &trill_pvp::game::bn1::AREJ_00,
    match_types: MATCH_TYPES,
    save_templates: &EXE1_T,
    logo_image: &EXE1_LOGO,
    background: BACKGROUND,
};

// ---------------- BN1 US ----------------
static BN1_SAVE: LazyLock<trill_dataview::game::bn1::save::Save> = LazyLock::new(|| {
    trill_dataview::game::bn1::save::Save::from_wram(
        include_bytes!("save/us.raw"),
        trill_dataview::game::bn1::save::GameInfo {
            region: trill_dataview::game::bn1::save::Region::US,
        },
    )
    .unwrap()
});
static BN1_T: SaveTemplates = LazyLock::new(|| vec![("", &*BN1_SAVE as &(dyn SaveTrait + Send + Sync))]);
static BN1_LOGO: LazyImage =
    LazyLock::new(|| image::load_from_memory(include_bytes!("../../logos/bn1-0.png")).unwrap());

pub static BN1: Game = Game {
    gamedb_entry: &trill_gamedb::AREE_00,
    hooks: &trill_pvp::game::bn1::AREE_00,
    match_types: MATCH_TYPES,
    save_templates: &BN1_T,
    logo_image: &BN1_LOGO,
    background: BACKGROUND,
};
