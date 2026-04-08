use crate::{
    display::list::{DisplayCommand, DisplayItem, DisplayList},
    resource::{
        assets::{AssetId, AssetsMap},
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
    },
    scene::script::CanvasCommand,
};

pub(crate) fn display_list_contains_video(list: &DisplayList, assets: &AssetsMap) -> bool {
    list.commands.iter().any(|command| match command {
        DisplayCommand::Draw {
            item: DisplayItem::Bitmap(bitmap),
        } => assets
            .path(&bitmap.asset_id)
            .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
            .unwrap_or(false),
        DisplayCommand::Draw {
            item: DisplayItem::Canvas(canvas),
        } => canvas.commands.iter().any(|command| {
            matches!(command, CanvasCommand::DrawImage { asset_id, .. }
                if assets
                    .path(&AssetId(asset_id.clone()))
                    .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
                    .unwrap_or(false))
        }),
        _ => false,
    })
}
