use crate::{ChordId, ChordTable, DiatonicChord, EditorMessage};
use iced::{
    advanced::{
        layout::{self, Layout},
        renderer,
        widget::{self, Widget},
        Clipboard, Shell,
    },
    alignment, keyboard, mouse, Background, Color, Element, Font, Length, Rectangle, Size, Theme,
};

// Define constants for the grid layout
const BUTTON_HEIGHT: f32 = 25.0;
const BUTTON_SPACING: f32 = 5.0;

// Define the rows of our grid. This is now the single source of truth for the grid's vertical layout.
pub const GRID_ROWS: &[(&str, &str)] = &[
    ("maj", ""),
    ("m", "m"),
    ("5", "5"),
    ("sus2", "sus2"),
    ("sus4", "sus4"),
    ("6", "6"),
    ("m6", "m6"),
    ("7", "7"),
    ("m7", "m7"),
    ("maj7", "maj7"),
    ("dim", "dim"),
    ("aug", "aug"),
    ("9", "9"),
    ("m9", "m9"),
    ("maj9", "maj9"),
    ("flat5", "b5"),
    ("m7b5", "m7b5"),
    ("dim7", "dim7"),
];

#[derive(Debug, Clone, Copy)]
enum ButtonStyle {
    Default,
    Diatonic,
    Playing,
    InversionTarget,
}

impl From<ButtonStyle> for Background {
    fn from(style: ButtonStyle) -> Self {
        let color = match style {
            ButtonStyle::Default => Color::from_rgb8(0x2E, 0x2E, 0x2E),
            ButtonStyle::Diatonic => Color::from_rgb8(0x4F, 0x4F, 0x4F),
            ButtonStyle::Playing => Color::from_rgb8(0x3A, 0x86, 0x5A),
            ButtonStyle::InversionTarget => Color::from_rgb8(0x3A, 0x5A, 0x86),
        };
        Background::Color(color)
    }
}

// The struct for our custom widget. It holds immutable references to the data it needs to draw.
// This makes it completely stateless and solves all borrowing issues.
pub struct ChordGrid<'a> {
    diatonics: &'a [DiatonicChord],
    chord_table: &'a ChordTable,
    playing_chord: &'a Option<ChordId>,
    inversion_chord: &'a Option<ChordId>,
}

impl<'a> ChordGrid<'a> {
    pub fn new(
        diatonics: &'a [DiatonicChord],
        chord_table: &'a ChordTable,
        playing_chord: &'a Option<ChordId>,
        inversion_chord: &'a Option<ChordId>,
    ) -> Self {
        Self {
            diatonics,
            chord_table,
            playing_chord,
            inversion_chord,
        }
    }
}

impl<'a> Widget<EditorMessage, renderer::Renderer> for ChordGrid<'a> {
    fn width(&self) -> Length {
        Length::Fill
    }

    fn height(&self) -> Length {
        Length::Fill
    }

    fn layout(&self, _renderer: &renderer::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits
            .width(self.width())
            .height(self.height())
            .resolve(Size::ZERO);
        layout::Node::new(size)
    }

    fn on_event(
        &mut self,
        _state: &mut widget::Tree,
        event: iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &renderer::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, EditorMessage>,
    ) -> iced::event::Status {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(cursor_pos) = cursor.position_in(layout.bounds()) {
                    let num_cols = self.diatonics.len();
                    if num_cols == 0 {
                        return iced::event::Status::Ignored;
                    }

                    let button_width =
                        (layout.bounds().width - (num_cols - 1) as f32 * BUTTON_SPACING)
                            / num_cols as f32;

                    let col_idx =
                        (cursor_pos.x / (button_width + BUTTON_SPACING)).floor() as usize;
                    let row_idx =
                        (cursor_pos.y / (BUTTON_HEIGHT + BUTTON_SPACING)).floor() as usize;

                    if row_idx < GRID_ROWS.len() && col_idx < num_cols {
                        let (type_key, _) = GRID_ROWS[row_idx];
                        let diatonic = &self.diatonics[col_idx];

                        let chord_id = ChordId {
                            root_note: diatonic.root_note.clone(),
                            chord_type: type_key.to_string(),
                        };

                        if _clipboard.modifiers().command() || _clipboard.modifiers().control() {
                            shell.publish(EditorMessage::SetInversionChord(chord_id));
                        } else {
                            shell.publish(EditorMessage::ChordPressed(chord_id));
                        }
                        return iced::event::Status::Captured;
                    }
                }
            }
            _ => {}
        }
        iced::event::Status::Ignored
    }

    fn draw(
        &self,
        _state: &widget::Tree,
        renderer: &mut renderer::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let num_cols = self.diatonics.len();
        if num_cols == 0 {
            return;
        }

        let bounds = layout.bounds();
        let button_width =
            (bounds.width - (num_cols - 1) as f32 * BUTTON_SPACING) / num_cols as f32;

        for (row_idx, (type_key, suffix)) in GRID_ROWS.iter().enumerate() {
            for (col_idx, d) in self.diatonics.iter().enumerate() {
                let root_note = &d.root_note;
                let chord_id = ChordId {
                    root_note: root_note.clone(),
                    chord_type: type_key.to_string(),
                };

                let is_valid_chord = self
                    .chord_table
                    .get(root_note)
                    .and_then(|vars| vars.get(*type_key))
                    .is_some();

                if !is_valid_chord {
                    continue;
                }

                let is_diatonic = d.chord_type == **type_key;

                let style = if self.playing_chord.as_ref() == Some(&chord_id) {
                    ButtonStyle::Playing
                } else if self.inversion_chord.as_ref() == Some(&chord_id) {
                    ButtonStyle::InversionTarget
                } else if is_diatonic {
                    ButtonStyle::Diatonic
                } else {
                    ButtonStyle::Default
                };

                let button_bounds = Rectangle {
                    x: bounds.x + col_idx as f32 * (button_width + BUTTON_SPACING),
                    y: bounds.y + row_idx as f32 * (BUTTON_HEIGHT + BUTTON_SPACING),
                    width: button_width,
                    height: BUTTON_HEIGHT,
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: button_bounds,
                        border_radius: 4.0.into(),
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                    },
                    style.into(),
                );

                let label = format!("{}{}", root_note, suffix);
                renderer.fill_text(iced::widget::text::Text {
                    content: &label,
                    bounds: button_bounds,
                    color: Color::WHITE,
                    size: 16.0,
                    font: Font::default(),
                    horizontal_alignment: alignment::Horizontal::Center,
                    vertical_alignment: alignment::Vertical::Center,
                    line_height: iced::widget::text::LineHeight::default(),
                    shaping: iced::widget::text::Shaping::Basic,
                });
            }
        }
    }
}

impl<'a, 'b> From<ChordGrid<'a>> for Element<'b, EditorMessage>
where
    'a: 'b,
{
    fn from(widget: ChordGrid<'a>) -> Self {
        Element::new(widget)
    }
}