use super::paint::ShaderSpec;
use super::Canvas2D;

pub enum RuntimeEffectChild<'a, C: Canvas2D + ?Sized> {
    Texture(&'a C::Image),
    Shader(ShaderSpec),
}
