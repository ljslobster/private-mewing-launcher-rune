use rune::compose::{Compositor, Frame};
use rune::render::{EffectPhase, EffectSite, KIND_EFFECT};
use rune_fx::{DistortParams, Fx, FxFrame, FxOptions, FxRenderer, FxSlot, LiquidMetalParams, Shape};

use crate::state::AppState;

pub const HERO_WAVE: u32 = 1;
pub const HERO_METAL: u32 = 2;

pub struct HeroWave {
    renderer: Option<FxRenderer>,
    level: f32,
    hover: bool,
    cursor: [f32; 2],
}

impl Default for HeroWave {
    fn default() -> Self {
        Self { renderer: None, level: 0.0, hover: false, cursor: [0.0; 2] }
    }
}

impl Compositor<AppState> for HeroWave {
    fn animating(&self) -> bool { self.level > 0.001 || self.hover }

    fn draws_own(&self) -> bool { self.level > 0.001 }

    fn render(&mut self, f: &mut Frame<'_, AppState>) {
        let mut at: Vec<u32> = Vec::new();
        for (n, instance) in f.rt.instances.iter().enumerate() {
            if instance.params[1] != KIND_EFFECT { continue; }
            let mark = f.rt.marks.get(n).copied().unwrap_or(0) & 0xff;
            if !matches!(mark, HERO_WAVE | HERO_METAL) { continue; }
            if instance.params[3] <= 0.0 { continue; }
            at.push(n as u32);
        }

        self.hover = f.rt.state.hero_hover && f.rt.state.hero_cursor.is_some() && !at.is_empty();
        if let Some(cursor) = f.rt.state.hero_cursor { self.cursor = cursor; }
        let target = if self.hover { 1.0 } else { 0.0 };
        let rate = 1.0 - (-(f.dt.max(0.0) as f32) / 0.18).exp();
        self.level += (target - self.level) * rate.clamp(0.0, 1.0);
        if (target - self.level).abs() < 0.002 { self.level = target; }

        if self.level <= 0.001 || at.is_empty() {
            f.draw_ui();
            return;
        }

        let t = f.rt.t as f32;
        let mut layers = Vec::new();
        let mut placements = Vec::new();
        for n in &at {
            let instance = &f.rt.instances[*n as usize];
            let scale = instance.params[3];
            let [x, y, _, _] = instance.rect.map(|v| v / scale);
            let center = [x + self.cursor[0], y + self.cursor[1]];
            let shape = Shape::circle(center[0], center[1], 110.0);
            let mark = f.rt.marks[*n as usize] & 0xff;
            let layer = if mark == HERO_METAL {
                Fx::fill(LiquidMetalParams::chrome()).on(shape).at(t).opacity(self.level * 0.45)
            } else {
                Fx::distort(DistortParams::Shockwave {
                    at: center,
                    reach: 85.0,
                    width: 12.0,
                    amount: 8.0,
                    chroma: 0.0,
                    t0: t - t.rem_euclid(1.15),
                    duration: 1.15,
                }).on(shape).at(t).opacity(self.level)
            };
            layers.push(layer);
            placements.push(FxSlot::default().scale(scale).clip(instance.clip, instance.fade));
        }

        let fx = self.renderer.get_or_insert_with(|| {
            FxRenderer::new(f.device, f.format, FxOptions { dither: true })
        });
        fx.set_srgb_blend(f.srgb_blend());
        fx.add_format(f.device, wgpu::TextureFormat::Rgba16Float);
        fx.prepare_with_slots(f.device, f.queue, &FxFrame {
            viewport: [f.size.0, f.size.1],
            scale: 1.0,
            layers: &layers,
            backdrop: None,
            sources: &[],
        }, &placements);

        let clear = f.rt.clear_color();
        f.ui.prepare(f.device, f.queue, &mut f.rt.text.atlas, &f.rt.instances, f.size);
        f.ui.draw_with_backdrop(f.encoder, f.target, clear, &mut |phase, site: &EffectSite<'_>| {
            let Some(index) = at.iter().position(|n| *n == site.index) else { return; };
            match phase {
                EffectPhase::Backdrop(_) => {
                    if let Some(backdrop) = site.backdrop {
                        fx.set_layer_backdrop(f.device, backdrop, index);
                    }
                }
                EffectPhase::Draw(pass) => fx.draw_layer_in(pass, site.format, site.origin, index),
            }
        });
    }
}
