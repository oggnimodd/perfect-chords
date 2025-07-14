use crossbeam_channel::{Receiver, Sender};
use nih_plug::prelude::*;
use nih_plug_egui::{EguiState, create_egui_editor, egui};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Deserialize, Debug, Clone)]
struct ChordVoicing {
    inversions: Vec<Vec<u8>>,
}

type ChordTable = HashMap<String, HashMap<String, ChordVoicing>>;

#[derive(Debug, Clone)]
struct DiatonicChord {
    root_note: String,
    chord_type: String,
    degree: String,
}

type ScaleMap = HashMap<String, Vec<DiatonicChord>>;

fn get_scale_map() -> ScaleMap {
    let mut scales = HashMap::new();
    let notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let major_pattern = [0, 2, 4, 5, 7, 9, 11];
    let minor_pattern = [0, 2, 3, 5, 7, 8, 10];
    let major_chord_types = ["maj", "m", "m", "maj", "maj", "m", "dim"];
    let minor_chord_types = ["m", "dim", "maj", "m", "m", "maj", "maj"];
    let major_degrees = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];
    let minor_degrees = ["i", "ii°", "III", "iv", "v", "VI", "VII"];

    for i in 0..12 {
        let major_key = format!("{} Major", notes[i]);
        let mut major_diatonics = Vec::new();
        for j in 0..7 {
            major_diatonics.push(DiatonicChord {
                root_note: notes[(i + major_pattern[j]) % 12].to_string(),
                chord_type: major_chord_types[j].to_string(),
                degree: major_degrees[j].to_string(),
            });
        }
        scales.insert(major_key, major_diatonics);

        let minor_key = format!("{} Minor", notes[i]);
        let mut minor_diatonics = Vec::new();
        for j in 0..7 {
            minor_diatonics.push(DiatonicChord {
                root_note: notes[(i + minor_pattern[j]) % 12].to_string(),
                chord_type: minor_chord_types[j].to_string(),
                degree: minor_degrees[j].to_string(),
            });
        }
        scales.insert(minor_key, minor_diatonics);
    }

    scales
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ChordId {
    root_note: String,
    chord_type: String,
}

enum MidiMessage {
    ChordOn(ChordId),
    ChordOff,
    SetInversionChord(ChordId),
    UpdateOctave(i8),
    UpdateInversion(u8),
    UpdateScale(String),
}

#[derive(Default, Clone)]
struct GuiState {
    octave: i8,
    scale: String,
    inversion: u8,
    playing_chord: Option<ChordId>,
    inversion_chord: Option<ChordId>,
}

pub struct PerfectChords {
    params: Arc<PerfectChordsParams>,
    midi_sender: Sender<MidiMessage>,
    midi_receiver: Receiver<MidiMessage>,
    active_notes: Vec<u8>,
    chord_table: ChordTable,
    scale_map: ScaleMap,
    state: GuiState,
}

#[derive(Params)]
pub struct PerfectChordsParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for PerfectChords {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let chord_table: ChordTable =
            serde_json::from_str(include_str!("../chords-builder/chords.json"))
                .expect("Failed to parse chords.json");

        Self {
            params: Arc::new(PerfectChordsParams::default()),
            midi_sender: sender,
            midi_receiver: receiver,
            active_notes: Vec::new(),
            chord_table,
            scale_map: get_scale_map(),
            state: GuiState {
                octave: 3,
                scale: "C Major".to_string(),
                inversion: 0,
                playing_chord: None,
                inversion_chord: None,
            },
        }
    }
}

impl Default for PerfectChordsParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(800, 600),
        }
    }
}

impl Plugin for PerfectChords {
    const NAME: &'static str = "Perfect Chords";
    const VENDOR: &'static str = "You";
    const URL: &'static str = "https://example.com";
    const EMAIL: &'static str = "user@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // Even for a MIDI-only plugin, hosts like LMMS expect an audio output bus
    // for instrument plugins. We'll define a silent stereo output.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: Some(new_nonzero_u32(2)),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let sender = self.midi_sender.clone();
        let initial_state = self.state.clone();
        let chord_table = self.chord_table.clone();
        let scale_map = self.scale_map.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            initial_state,
            |_, _| {},
            move |egui_ctx, _setter, state| {
                let grid_rows: Vec<(&str, &str)> = vec![
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
                    ("flat5", "5-"),
                    ("m7b5", "m7b5"),
                    ("dim7", "dim7"),
                ];

                let diatonics = scale_map.get(&state.scale).cloned().unwrap_or_default();

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(4.0, 4.0);
                    ui.style_mut().spacing.item_spacing = egui::vec2(2.0, 2.0);

                    ui.horizontal(|ui| {
                        ui.label("Scale:");
                        egui::ComboBox::from_id_salt("scale_picker")
                            .selected_text(&state.scale)
                            .show_ui(ui, |ui| {
                                let mut sorted_scales: Vec<_> = scale_map.keys().collect();
                                sorted_scales.sort();
                                for scale_name in sorted_scales {
                                    if ui
                                        .selectable_value(
                                            &mut state.scale,
                                            scale_name.clone(),
                                            scale_name,
                                        )
                                        .clicked()
                                    {
                                        let _ = sender
                                            .send(MidiMessage::UpdateScale(scale_name.clone()));
                                    }
                                }
                            });

                        ui.add_space(20.0);
                        ui.label("Octave:");
                        if ui.button("◀").clicked() {
                            state.octave -= 1;
                            let _ = sender.send(MidiMessage::UpdateOctave(state.octave));
                        }
                        ui.label(format!("{}", state.octave));
                        if ui.button("▶").clicked() {
                            state.octave += 1;
                            let _ = sender.send(MidiMessage::UpdateOctave(state.octave));
                        }

                        ui.add_space(20.0);
                        ui.label("Inversion:");
                        if ui.button("◀").clicked() {
                            if state.inversion > 0 {
                                state.inversion -= 1;
                                let _ = sender.send(MidiMessage::UpdateInversion(state.inversion));
                            }
                        }
                        ui.label(format!("{}", state.inversion));
                        if ui.button("▶").clicked() {
                            state.inversion += 1;
                            let _ = sender.send(MidiMessage::UpdateInversion(state.inversion));
                        }
                    });

                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("chord_grid").show(ui, |ui| {
                            ui.label("");
                            for d in &diatonics {
                                ui.strong(&d.degree);
                            }
                            ui.end_row();

                            for (type_key, suffix) in grid_rows {
                                ui.label("");
                                for d in &diatonics {
                                    let root_note = &d.root_note;
                                    let chord_id = ChordId {
                                        root_note: root_note.clone(),
                                        chord_type: type_key.to_string(),
                                    };

                                    if chord_table
                                        .get(root_note)
                                        .and_then(|vars| vars.get(type_key))
                                        .is_some()
                                    {
                                        let label = format!("{}{}", root_note, suffix);
                                        let is_playing =
                                            state.playing_chord.as_ref() == Some(&chord_id);
                                        let is_inversion_target =
                                            state.inversion_chord.as_ref() == Some(&chord_id);
                                        let is_diatonic = d.chord_type == type_key;

                                        let button_color = if is_playing {
                                            egui::Color32::from_rgb(100, 200, 100)
                                        } else if is_inversion_target {
                                            egui::Color32::from_rgb(100, 150, 255)
                                        } else if is_diatonic {
                                            ui.visuals().widgets.inactive.bg_fill
                                        } else {
                                            ui.visuals().widgets.noninteractive.bg_fill
                                        };

                                        let button = egui::Button::new(label)
                                            .min_size(egui::vec2(ui.available_width(), 20.0))
                                            .fill(button_color);

                                        let response = ui.add(button);

                                        if response.hovered()
                                            && egui_ctx.input(|i| i.pointer.primary_down())
                                        {
                                            if egui_ctx.input(|i| i.modifiers.ctrl) {
                                                if response.clicked() {
                                                    state.inversion_chord = Some(chord_id.clone());
                                                    let _ = sender.send(
                                                        MidiMessage::SetInversionChord(chord_id),
                                                    );
                                                }
                                            } else if state.playing_chord.as_ref()
                                                != Some(&chord_id)
                                            {
                                                state.playing_chord = Some(chord_id.clone());
                                                let _ = sender.send(MidiMessage::ChordOn(chord_id));
                                            }
                                        }
                                    } else {
                                        ui.label("");
                                    }
                                }
                                ui.end_row();
                            }
                        });
                    });

                    if state.playing_chord.is_some()
                        && egui_ctx.input(|i| i.pointer.primary_released())
                    {
                        state.playing_chord = None;
                        let _ = sender.send(MidiMessage::ChordOff);
                    }
                });
            },
        )
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        while let Ok(message) = self.midi_receiver.try_recv() {
            match message {
                MidiMessage::ChordOn(chord_id) => {
                    for note in self.active_notes.drain(..) {
                        context.send_event(NoteEvent::NoteOff {
                            timing: 0,
                            voice_id: None,
                            channel: 0,
                            note,
                            velocity: 0.0,
                        });
                    }

                    let target_chord_id = self.state.inversion_chord.as_ref().unwrap_or(&chord_id);
                    if let Some(voicing) = self
                        .chord_table
                        .get(&target_chord_id.root_note)
                        .and_then(|v| v.get(&target_chord_id.chord_type))
                    {
                        let num_inversions = voicing.inversions.len();
                        if num_inversions > 0 {
                            let inversion_idx = self.state.inversion as usize % num_inversions;
                            let notes_to_play = &voicing.inversions[inversion_idx];
                            let octave_offset = (self.state.octave - 3) * 12;

                            for note in notes_to_play {
                                let final_note = (*note as i16 + octave_offset as i16) as u8;
                                context.send_event(NoteEvent::NoteOn {
                                    timing: 0,
                                    voice_id: None,
                                    channel: 0,
                                    note: final_note,
                                    velocity: 0.8,
                                });
                                self.active_notes.push(final_note);
                            }
                        }
                    }
                    self.state.playing_chord = Some(chord_id);
                }
                MidiMessage::ChordOff => {
                    for note in self.active_notes.drain(..) {
                        context.send_event(NoteEvent::NoteOff {
                            timing: 0,
                            voice_id: None,
                            channel: 0,
                            note,
                            velocity: 0.0,
                        });
                    }
                    self.state.playing_chord = None;
                }
                MidiMessage::SetInversionChord(chord_id) => {
                    self.state.inversion_chord = Some(chord_id);
                }
                MidiMessage::UpdateOctave(octave) => {
                    self.state.octave = octave;
                }
                MidiMessage::UpdateInversion(inversion) => {
                    self.state.inversion = inversion;
                }
                MidiMessage::UpdateScale(scale) => {
                    self.state.scale = scale;
                }
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for PerfectChords {
    const CLAP_ID: &'static str = "com.you.perfect-chords";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A chord generation and MIDI player plugin.");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::NoteEffect,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for PerfectChords {
    const VST3_CLASS_ID: [u8; 16] = *b"PerfectChordsVST";
    // Removed `Fx` as it's not an audio effect, which can confuse some hosts.
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(PerfectChords);
nih_export_vst3!(PerfectChords);
