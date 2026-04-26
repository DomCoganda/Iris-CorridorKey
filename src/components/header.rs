use kairos::*;
use kairos::row;

#[component]
pub struct Header;

impl Component for Header {
    fn build(&self, _palette: &Palette) -> Widget {
        row![
            width: Fill,
            height: Custom(60.0),
            padding: Padding::none(),
            children![
                icon![
                    source: Path(Embedded("../assets/logo.svg")),
                    size: Custom(60.0),
                ],
                text![
                    content: "IRIS - CorridorKey",
                    style: TextStyle::H2,
                ],
                spacer![
                    size: Fill,
                    orientation: Orientation::Horizontal,
                ],
            ],
        ]
    }
}

impl Header {
    pub fn new() -> Self {
        Header
    }
}