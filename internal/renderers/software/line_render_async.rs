//! Async line render

use crate::{
    PhysicalRect, PhysicalRegion, PhysicalSize, SoftwareRenderer, draw_functions, prepare_scene,
};

use super::scene::*;
use i_slint_core::Brush;
use i_slint_core::lengths::{PointLengths, SizeLengths};
use i_slint_core::window::WindowInner;

use super::draw_functions::TargetPixel;

/// A trait that provides line buffers for asynchronous rendering.
pub trait LineBufferProviderAsync {
    /// The pixel type of the buffer
    type TargetPixel: TargetPixel;

    /// Called once per line, you will have to call the render_fn back with the buffer.
    ///
    /// The `line` is the y position of the line to be drawn.
    /// The `range` is the range within the line that is going to be rendered (eg, within the dirty region)
    /// The `render_fn` function should be called to render the line, passing the buffer
    /// corresponding to the specified line and range.
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) -> impl Future<Output = ()>;
}

/// Renders a window frame line by line asynchronously, using the provided line buffer provider.
pub async fn render_window_frame_by_line_async<LB: LineBufferProviderAsync>(
    window: &WindowInner,
    background: Brush,
    size: PhysicalSize,
    renderer: &SoftwareRenderer,
    mut line_buffer: LB,
) -> PhysicalRegion {
    let mut scene = prepare_scene(window, size, renderer);

    let to_draw_tr = scene.dirty_region.bounding_rect();

    let mut background_color = <LB as LineBufferProviderAsync>::TargetPixel::background();
    // FIXME gradient
    TargetPixel::blend(&mut background_color, background.color().into());

    while scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
        for r in &scene.current_line_ranges {
            line_buffer
                .process_line(
                    scene.current_line.get() as usize,
                    r.start as usize..r.end as usize,
                    |line_buffer| {
                        let offset = r.start;

                        // line_buffer.fill(background_color);
                        for span in scene.items[0..scene.current_items_index].iter().rev() {
                            debug_assert!(scene.current_line >= span.pos.y_length());
                            debug_assert!(
                                scene.current_line
                                    < span.pos.y_length() + span.size.height_length(),
                            );
                            if span.pos.x >= r.end {
                                continue;
                            }
                            let begin = r.start.max(span.pos.x);
                            let end = r.end.min(span.pos.x + span.size.width);
                            if begin >= end {
                                continue;
                            }

                            let extra_left_clip = begin - span.pos.x;
                            let extra_right_clip = span.pos.x + span.size.width - end;
                            let range_buffer = &mut line_buffer
                                [(begin - offset) as usize..(end - offset) as usize];

                            match span.command {
                                SceneCommand::Rectangle { color } => {
                                    TargetPixel::blend_slice(range_buffer, color);
                                }
                                SceneCommand::Texture { texture_index } => {
                                    let texture = &scene.vectors.textures[texture_index as usize];
                                    draw_functions::draw_texture_line(
                                        &PhysicalRect { origin: span.pos, size: span.size },
                                        scene.current_line,
                                        texture,
                                        range_buffer,
                                        extra_left_clip,
                                        extra_right_clip,
                                    );
                                }
                                SceneCommand::SharedBuffer { shared_buffer_index } => {
                                    let texture = scene.vectors.shared_buffers
                                        [shared_buffer_index as usize]
                                        .as_texture();
                                    draw_functions::draw_texture_line(
                                        &PhysicalRect { origin: span.pos, size: span.size },
                                        scene.current_line,
                                        &texture,
                                        range_buffer,
                                        extra_left_clip,
                                        extra_right_clip,
                                    );
                                }
                                SceneCommand::RoundedRectangle { rectangle_index } => {
                                    let rr =
                                        &scene.vectors.rounded_rectangles[rectangle_index as usize];
                                    draw_functions::draw_rounded_rectangle_line(
                                        &PhysicalRect { origin: span.pos, size: span.size },
                                        scene.current_line,
                                        rr,
                                        range_buffer,
                                        extra_left_clip,
                                        extra_right_clip,
                                    );
                                }
                                SceneCommand::LinearGradient { linear_gradient_index } => {
                                    let g = &scene.vectors.linear_gradients
                                        [linear_gradient_index as usize];

                                    draw_functions::draw_linear_gradient(
                                        &PhysicalRect { origin: span.pos, size: span.size },
                                        scene.current_line,
                                        g,
                                        range_buffer,
                                        extra_left_clip,
                                    );
                                }
                                SceneCommand::RadialGradient { radial_gradient_index } => {
                                    let g = &scene.vectors.radial_gradients
                                        [radial_gradient_index as usize];
                                    draw_functions::draw_radial_gradient(
                                        &PhysicalRect { origin: span.pos, size: span.size },
                                        scene.current_line,
                                        g,
                                        range_buffer,
                                        extra_left_clip,
                                        extra_right_clip,
                                    );
                                }
                                SceneCommand::ConicGradient { conic_gradient_index } => {
                                    let g = &scene.vectors.conic_gradients
                                        [conic_gradient_index as usize];
                                    draw_functions::draw_conic_gradient(
                                        &PhysicalRect { origin: span.pos, size: span.size },
                                        scene.current_line,
                                        g,
                                        range_buffer,
                                        extra_left_clip,
                                        extra_right_clip,
                                    );
                                }
                            }
                        }
                    },
                )
                .await;
        }

        if scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
            scene.next_line();
        }
    }
    scene.dirty_region
}
