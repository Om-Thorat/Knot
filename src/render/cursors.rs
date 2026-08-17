use smithay::{
    backend::renderer::{
        element::{solid::SolidColorRenderElement, Id, Kind},
        utils::CommitCounter,
        Color32F,
    },
    utils::{Logical, Point, Rectangle, Size},
};

pub struct MultiplayerCursorOverlay;

impl MultiplayerCursorOverlay {
    pub fn create_cursor_element(
        location: Point<f64, Logical>,
        color: [f32; 4],
    ) -> SolidColorRenderElement {
        let x = location.x.round() as i32;
        let y = location.y.round() as i32;

        let geometry = Rectangle::new(
            Point::from((x, y)),
            Size::from((14, 14)),
        );

        SolidColorRenderElement::new(
            Id::new(),
            geometry,
            CommitCounter::default(),
            Color32F::new(color[0], color[1], color[2], color[3]),
            Kind::Unspecified,
        )
    }
}
