use bevy::asset::{AssetApp, AssetPlugin, LoadState, UnapprovedPathMode};
use bevy::prelude::*;
use bevy_kira_audio::prelude::Mp3Loader;
use bevy_kira_audio::AudioSource;

#[test]
fn mp3_fixture_decodes_through_bevy_asset_loader() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            unapproved_path_mode: UnapprovedPathMode::Allow,
            ..default()
        },
    ));
    app.init_asset::<AudioSource>();
    app.register_asset_loader(Mp3Loader);

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dtx-core/tests/fixtures/compat-tone.mp3");
    let handle: Handle<AudioSource> = app
        .world()
        .resource::<AssetServer>()
        .load(path.to_string_lossy().into_owned());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.update();
        match app
            .world()
            .resource::<AssetServer>()
            .get_load_state(handle.id())
        {
            Some(LoadState::Loaded) => return,
            Some(LoadState::Failed(error)) => panic!("MP3 decode failed: {error:?}"),
            _ if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            _ => panic!("MP3 fixture did not reach a terminal load state"),
        }
    }
}
