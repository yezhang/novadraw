use super::buffer::hb_buffer_t;
use super::hb_font_t;
use super::ot_layout::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;

pub fn substitute<P: super::face::shape_admission::ShapeAdmission>(
    plan: &hb_ot_shape_plan_t, face: &hb_font_t, buffer: &mut hb_buffer_t, policy: &mut P,
) -> Result<(), P::Error> {
    apply_layout_table_with(plan, face, buffer, face.ot_tables.gsub.as_ref(), policy)
}
