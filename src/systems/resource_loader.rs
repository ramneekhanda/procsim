use bevy::prelude::*;

use crate::c_log;
use crate::resources::common_assets::{CommonAssets, LoadingState, LoadingStateOpt, ResourceType};
use crate::resources::graph_def::GraphChange;
use crate::resources::graph_def::GraphDefinitionRes;

// Bundled locally under assets/fonts/ rather than fetched from
// raw.githubusercontent.com at runtime - same reasoning as
// "default_system_icon" below: some proxies block that host even when the
// rest of GitHub is reachable, and this is the app-wide default font every
// graph that doesn't set its own `graph_attrs.font` falls back to. A plain
// relative path here resolves the same way on both platforms: natively
// against the crate-root `assets/` dir (Bevy's default filesystem asset
// root), and in the browser against `web/static/assets` - a checked-in
// symlink to that same directory, so SvelteKit's dev server and its
// adapter-static production build both serve it at the page's own origin
// without a separate copy step.
const DEFAULT_FONT_URL: &str = "fonts/ComicNeue-Regular.ttf";

fn load_default_fonts_and_icons(ca: &mut ResMut<CommonAssets>, asset_server: &Res<AssetServer>) {
    ca.resource_map.insert(
        "default_font".to_string(),
        ResourceType::FontHandle(asset_server.load(DEFAULT_FONT_URL)),
    );
    ca.resource_map.insert(
      "default_system_icon".to_string(),
      // Bundled locally under assets/icons/ (see NOTICE.md there) rather than
      // fetched from raw.githubusercontent.com at runtime - some proxies
      // block that host even when the rest of GitHub is reachable, which
      // broke this unconditional default (every node without its own icon)
      // for anyone behind one.
      ResourceType::ImageHandle( asset_server.load("icons/Servers.png"))
    );
}

fn load_resources_from_file(
    ca: &mut ResMut<CommonAssets>,
    asset_server: &Res<AssetServer>,
    g: &Res<GraphDefinitionRes>,
) {
    c_log!("load_resources_from_file: found {} icons", g.graph_defn.icons.len());
    for icon in &g.graph_defn.icons {
        c_log!("  inserting icon '{}' -> '{}'", icon.id, icon.url);
        ca.resource_map.insert(
            icon.id.clone(),
            ResourceType::ImageHandle(asset_server.load(icon.url.clone())),
        );
    }

    // `graph_attrs.font` is part of the theme (see GraphAttrs' doc comment) -
    // swap the single shared `"default_font"` handle to match whenever it
    // actually changes: a theme that sets one, back to the hardcoded app
    // default when a newly-loaded graph doesn't set one at all, or a no-op
    // when it's unchanged from last time (comparing against `theme_font_url`
    // avoids re-issuing `asset_server.load` - and the resulting `LoadingState`
    // gate re-triggering - on every single graph reload).
    let wanted_font = g.graph_defn.graph_attrs.font.clone();
    if wanted_font != ca.theme_font_url {
        let url = wanted_font.clone().unwrap_or_else(|| DEFAULT_FONT_URL.to_string());
        c_log!("theme font changed -> '{}'", url);
        ca.resource_map
            .insert("default_font".to_string(), ResourceType::FontHandle(asset_server.load(url)));
        ca.theme_font_url = wanted_font;
    }
}

pub fn load_assets(
    mut ca: ResMut<CommonAssets>,
    mut ls: ResMut<LoadingState>,
    g: Res<GraphDefinitionRes>,
    asset_server: Res<AssetServer>,
    mut event_writer: EventWriter<GraphChange>,
) {
    if g.is_changed() || g.is_added() {
        if !ca.resource_map.contains_key("default_font")
            || !ca.resource_map.contains_key("default_system_icon")
        {
            load_default_fonts_and_icons(&mut ca, &asset_server);
        }
        load_resources_from_file(&mut ca, &asset_server, &g);
    }

    use bevy::asset::LoadState;
    let mut still_loading: bool = false;
    let mut failed_res_vec: Vec<String> = Vec::default();

    for (res, asset) in ca.resource_map.iter() {
        let uh: UntypedHandle;
        match asset {
            ResourceType::FontHandle(f) => uh = f.clone().into(),
            ResourceType::ImageHandle(f) => uh = f.clone().into(),
        }

        match asset_server.get_load_state(uh.id()) {
            Some(LoadState::Failed(x)) => {
                // one of our assets had an error
                c_log!("asset load error");
                c_log!("{}", x.to_string());
                failed_res_vec.push(res.clone());
            }
            Some(LoadState::Loaded) => {
                // all assets are now ready
            }
            _ => {
                // NotLoaded/Loading/None: not fully ready yet
                still_loading = true;
            }
        }
    }

    //remove failed assets
    for res in failed_res_vec.iter() {
        ca.resource_map.remove(res);
    }

    if still_loading != true {
        ls.state = LoadingStateOpt::Ready;
        event_writer.send(GraphChange {});
        c_log!("all resources ready");
    }
}
