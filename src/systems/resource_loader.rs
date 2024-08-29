use bevy::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}
use crate::resources::common_assets::{CommonAssets, LoadingState, LoadingStateOpt, ResourceType};
use crate::resources::graph_def::GraphDefinitionRes;

fn load_default_fonts_and_icons(ca: &mut ResMut<CommonAssets>, asset_server: &Res<AssetServer>) {
    ca.resource_map.insert(
        "default_font".to_string(),
        ResourceType::FontHandle(
            asset_server.load("http://fonts.gstatic.com/s/abeezee/v9/mE5BOuZKGln_Ex0uYKpIaw.ttf"),
        ),
    );
    ca.resource_map.insert(
      "default_system_icon".to_string(), 
      ResourceType::ImageHandle( asset_server.load("https://raw.githubusercontent.com/awslabs/aws-icons-for-plantuml/main/dist/General/Servers.png"))
    );
}

fn load_resources_from_file(
    ca: &mut ResMut<CommonAssets>,
    asset_server: &Res<AssetServer>,
    g: &Res<GraphDefinitionRes>,
) {
    for node in &g.graph_defn.nodes {
        if node.attrs.icon.is_some() {
            let icon_string = node.attrs.icon.clone().unwrap();
            ca.resource_map.insert(
                icon_string.clone(),
                ResourceType::ImageHandle(asset_server.load(icon_string)),
            );
        }
    }
}

pub fn load_assets(
    mut ca: ResMut<CommonAssets>,
    mut ls: ResMut<LoadingState>,
    g: Res<GraphDefinitionRes>,
    asset_server: Res<AssetServer>,
) {
    if g.is_changed() || g.is_added() {
        ca.resource_map.clear();
        load_default_fonts_and_icons(&mut ca, &asset_server);
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

        match asset_server.get_load_state(uh.id()).unwrap() {
            LoadState::Failed(x) => {
                // one of our assets had an error
                log("asset load error");
                log(&x.to_string());
                failed_res_vec.push(res.clone());
            }
            LoadState::Loaded => {
                // all assets are now ready

                // this might be a good place to transition into your in-game state

                // remove the resource to drop the tracking handles
                //commands.remove_resource::<AssetsLoading>();
                // (note: if you don't have any other handles to the assets
                // elsewhere, they will get unloaded after this)
            }
            _ => {
                // NotLoaded/Loading: not fully ready yet
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
        log("all resources ready");
    }
}
