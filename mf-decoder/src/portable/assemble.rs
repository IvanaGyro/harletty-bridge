//! The engine, assembled in this library (plan D10): the Dolby and DTS
//! families are linked in as plugins of their own, so no bridge DLL is loaded.
//!
//! This follows `Engine::from_paths` in Omniphony at the commit pinned in
//! `.omniphony-ref` (mgth/Omniphony 55af2c5), from the config load to
//! `Engine::new`, minus what a decoder inside a player has no use for: the
//! OSC/save bookkeeping (config path, bridge paths, input path, profiles
//! view), the overlay preferences, the live-handoff sidecar and the bridge
//! discovery. The folder lookup of the bridge DLLs is gone with them.
//! `from_paths` itself stays in the tests, as the reference this must equal.

use std::path::Path;
use std::sync::OnceLock;

use bridge_api::BridgeLibRef;
use bridge_common::plugin::{self, Plugin};
use bridge_family_dolby::DolbyPipeline;
use bridge_family_dts::DtsPipeline;
use orender_engine::bridge_loader::{BridgeLibs, LoadedBridge, configure_presentation};
use orender_engine::renderer_build::{
    SpatialRendererParams, build_spatial_renderer, seed_runtime_state_from_render_config,
};
use orender_engine::{Config, Engine};
use renderer::config::ConfigLoadStatus;
use renderer::speaker_layout::SpeakerLayout;

/// The monitoring cadences `Engine::from_paths` gives an embedded host.
const METER_RATE_HZ: f32 = 10.0;
const DIAG_RATE_HZ: f32 = 10.0;

struct Dolby;

impl Plugin for Dolby {
    type Family = DolbyPipeline;
    const NAME: &'static str = concat!(env!("CARGO_PKG_NAME"), " (Dolby family)");
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
}

struct Dts;

impl Plugin for Dts {
    type Family = DtsPipeline;
    const NAME: &'static str = concat!(env!("CARGO_PKG_NAME"), " (DTS family)");
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
}

/// The two linked-in families, once per process: each root module is leaked
/// when made.
pub fn libs() -> BridgeLibs {
    static DOLBY: OnceLock<BridgeLibRef> = OnceLock::new();
    static DTS: OnceLock<BridgeLibRef> = OnceLock::new();
    let dolby = *DOLBY.get_or_init(plugin::root_module::<Dolby>);
    let dts = *DTS.get_or_init(plugin::root_module::<Dts>);
    BridgeLibs::new(vec![dolby, dts]).expect("two plugins")
}

/// An engine for one stream: `config` (Omniphony's `config.yaml` schema, or
/// the defaults), rendering to the speakers of `layout`, decoding `input_codec`
/// at `sample_rate`.
pub fn engine(
    config: Option<&Path>,
    layout: &Path,
    input_codec: &str,
    sample_rate: u32,
) -> Result<Engine, String> {
    build(config, layout, input_codec, sample_rate).map_err(|e| format!("{e:#}"))
}

fn build(
    config: Option<&Path>,
    layout: &Path,
    input_codec: &str,
    sample_rate: u32,
) -> anyhow::Result<Engine> {
    let t_total = std::time::Instant::now();
    let mut render_cfg = match config {
        Some(path) => {
            // Not `load_or_default_with_live`: a live-handoff sidecar is the
            // state of another process, and this one has no OSC to leave one.
            let (cfg, status) = Config::load_or_default_with_status(path);
            match status {
                ConfigLoadStatus::Loaded => {}
                ConfigLoadStatus::NewerSchema => log::warn!(
                    "config '{}' was written by a newer Omniphony; running on what this build \
                     understands of it",
                    path.display()
                ),
                _ => log::warn!(
                    "config '{}' not loaded ({}); running on built-in defaults",
                    path.display(),
                    status.as_str()
                ),
            }
            cfg.render
        }
        None => None,
    };
    let layout = SpeakerLayout::from_file(layout)?;

    let t_bridge = std::time::Instant::now();
    let mut bridge = LoadedBridge::open(libs())?;
    let presentation = render_cfg
        .as_ref()
        .and_then(renderer::config_fields::presentation::get)
        .map(|p| p.to_string())
        .unwrap_or_else(|| renderer::config_fields::presentation::DEFAULT.to_string());
    if let Err(e) = configure_presentation(&mut bridge.bridge, &presentation) {
        // A decoder inside a player: the bridge's own default still plays.
        log::warn!("{e}; keeping the bridge default");
    }
    // Disambiguates the bridge's `Raw` transport (no data_type byte).
    bridge.configure("input_codec", input_codec);
    let vbap_defaults = bridge.vbap_cartesian_defaults();
    let preferred = bridge.preferred_vbap_table_mode();
    log::info!(
        "bridge configured in {:.2}s",
        t_bridge.elapsed().as_secs_f64()
    );
    if let Some(cfg) = render_cfg.as_mut() {
        renderer::evaluation_grid::settle_config(
            cfg,
            renderer::evaluation_grid::EvaluationGrid::from_hint(vbap_defaults, preferred),
        );
    }

    let params = SpatialRendererParams::from_render_config(render_cfg.as_ref());
    let mut renderer = build_spatial_renderer(
        &params,
        layout,
        sample_rate,
        vbap_defaults,
        preferred,
        render_cfg.as_ref(),
    )?;
    let control = renderer.renderer_control();
    control.set_cadence_defaults_hz(METER_RATE_HZ, DIAG_RATE_HZ);
    seed_runtime_state_from_render_config(&control, render_cfg.as_ref());
    let supported_drc: Vec<String> = bridge
        .bridge
        .supported_drc_modes()
        .iter()
        .map(|m| m.as_str().to_string())
        .collect();
    control.set_bridge_supported_drc_modes(supported_drc);
    renderer.prepare_speaker_stage()?;

    let engine = Engine::new(bridge, renderer, sample_rate);
    log::info!("engine assembled in {:.2?}", t_total.elapsed());
    Ok(engine)
}
