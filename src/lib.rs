use crossbeam_channel::{Receiver, Sender};
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
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

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
struct ChordId {
    root_note: String,
    chord_type: String,
}

enum MidiMessage {
    ChordOn(ChordId),
    ChordOff,
    SetInversionChord(ChordId),
    UpdateOctave(i8),
    UpdateInversion(ChordId, u8),
    UpdateScale(String),
    UpdateKeyMapping(egui::Key, ChordId),
    KeyChordOn(egui::Key),
    KeyChordOff(egui::Key),
}

#[derive(Clone)]
struct GuiState {
    octave: i8,
    root_note: String,
    scale_type: String,

    playing_chord: Option<ChordId>,
    inversion_chord: Option<ChordId>,
    inversion_map: HashMap<ChordId, u8>,

    key_mappings: HashMap<egui::Key, ChordId>,
    playing_key: Option<egui::Key>,
    view_mode: ViewMode,
    key_to_map: Option<egui::Key>,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            octave: 3,
            root_note: "C".to_string(),
            scale_type: "Major".to_string(),
            playing_chord: None,
            inversion_chord: None,
            inversion_map: HashMap::new(),
            key_mappings: generate_default_key_mappings(&get_scale_map(), "C Major".to_string()),
            playing_key: None,
            view_mode: ViewMode::ChordGrid,
            key_to_map: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    ChordGrid,
    KeyMapping,
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::ChordGrid
    }
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
            state: GuiState::default(),
        }
    }
}

fn generate_default_key_mappings(scale_map: &ScaleMap, current_scale: String) -> HashMap<egui::Key, ChordId> {
    let mut mappings = HashMap::new();
    let default_keys = [
        egui::Key::Z,
        egui::Key::X,
        egui::Key::C,
        egui::Key::V,
        egui::Key::B,
        egui::Key::N,
        egui::Key::M,
    ];

    if let Some(diatonics) = scale_map.get(&current_scale) {
        for (i, key) in default_keys.iter().enumerate() {
            if let Some(diatonic_chord) = diatonics.get(i) {
                mappings.insert(
                    *key,
                    ChordId {
                        root_note: diatonic_chord.root_note.clone(),
                        chord_type: diatonic_chord.chord_type.clone(),
                    },
                );
            }
        }
    }
    mappings
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

                let scale = format!("{} {}", state.root_note, state.scale_type);
                let diatonics = scale_map.get(&scale).cloned().unwrap_or_default();

                egui_ctx.input(|i| {
                    for event in &i.events {
                        if let egui::Event::Key {
                            key,
                            pressed,
                            repeat,
                            ..
                        } = event
                        {
                            if !*repeat && state.key_mappings.contains_key(key) {
                                if *pressed {
                                    if state.playing_key != Some(*key) {
                                        state.playing_key = Some(*key);
                                        state.playing_chord = state.key_mappings.get(key).cloned();
                                        let _ = sender.send(MidiMessage::KeyChordOn(*key));
                                    }
                                } else if state.playing_key == Some(*key) {
                                    state.playing_key = None;
                                    state.playing_chord = None;
                                    let _ = sender.send(MidiMessage::KeyChordOff(*key));
                                }
                            }
                        }
                    }
                });

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(4.0, 4.0);
                    ui.style_mut().spacing.item_spacing = egui::vec2(2.0, 2.0);

                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut state.view_mode, ViewMode::ChordGrid, "Chord Grid");
                        ui.selectable_value(&mut state.view_mode, ViewMode::KeyMapping, "Key Mapping");
                    });

                    ui.separator();

                    match state.view_mode {
                        ViewMode::ChordGrid => {
                            ui.horizontal(|ui| {
                                ui.label("Root Note:");
                                egui::ComboBox::from_id_salt("root_note_picker")
                                    .selected_text(&state.root_note)
                                    .show_ui(ui, |ui| {
                                        for note in ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"].iter() {
                                            if ui
                                                .selectable_value(
                                                    &mut state.root_note,
                                                    note.to_string(),
                                                    *note,
                                                )
                                                .clicked()
                                            {
                                                let new_scale = format!("{} {}", state.root_note, state.scale_type);
                                                state.key_mappings = generate_default_key_mappings(&scale_map, new_scale.clone());
                                                let _ = sender.send(MidiMessage::UpdateScale(new_scale));
                                            }
                                        }
                                    });

                                ui.label("Scale Type:");
                                egui::ComboBox::from_id_salt("scale_type_picker")
                                    .selected_text(&state.scale_type)
                                    .show_ui(ui, |ui| {
                                        for scale_type in ["Major", "Minor"].iter() {
                                            if ui
                                                .selectable_value(
                                                    &mut state.scale_type,
                                                    scale_type.to_string(),
                                                    *scale_type,
                                                )
                                                .clicked()
                                            {
                                                let new_scale = format!("{} {}", state.root_note, state.scale_type);
                                                state.key_mappings = generate_default_key_mappings(&scale_map, new_scale.clone());
                                                let _ = sender.send(MidiMessage::UpdateScale(new_scale));
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
                                let current_inversion = state.inversion_chord.as_ref()
                                    .and_then(|chord_id| state.inversion_map.get(chord_id))
                                    .copied()
                                    .unwrap_or(0);

                                if ui.button("◀").clicked() {
                                    if let Some(chord_id) = state.inversion_chord.clone() {
                                        let voicing = chord_table
                                            .get(&chord_id.root_note)
                                            .and_then(|v| v.get(&chord_id.chord_type));
                                        if let Some(voicing) = voicing {
                                            let num_inversions = voicing.inversions.len() as u8;
                                            if num_inversions > 0 {
                                                let new_inversion = (current_inversion + num_inversions - 1) % num_inversions;
                                                state.inversion_map.insert(chord_id.clone(), new_inversion);
                                                let _ = sender.send(MidiMessage::UpdateInversion(chord_id, new_inversion));
                                            }
                                        }
                                    }
                                }
                                ui.label(format!("{}", current_inversion));
                                if ui.button("▶").clicked() {
                                    if let Some(chord_id) = state.inversion_chord.clone() {
                                        let voicing = chord_table
                                            .get(&chord_id.root_note)
                                            .and_then(|v| v.get(&chord_id.chord_type));
                                        if let Some(voicing) = voicing {
                                            let num_inversions = voicing.inversions.len() as u8;
                                            if num_inversions > 0 {
                                                let new_inversion = (current_inversion + 1) % num_inversions;
                                                state.inversion_map.insert(chord_id.clone(), new_inversion);
                                                let _ = sender.send(MidiMessage::UpdateInversion(chord_id, new_inversion));
                                            }
                                        }
                                    }
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

                                    for &(type_key, suffix) in &grid_rows {
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
                                                let is_playing_mouse =
                                                    state.playing_chord.as_ref() == Some(&chord_id);
                                                let is_inversion_target =
                                                    state.inversion_chord.as_ref() == Some(&chord_id);
                                                let is_diatonic = d.chord_type == type_key;
                                                
                                                let is_key_active = state.playing_key.as_ref().and_then(|k| state.key_mappings.get(k)) == Some(&chord_id);


                                                let button_color = if is_playing_mouse || is_key_active {
                                                    egui::Color32::from_rgb(100, 200, 100)
                                                } else if is_inversion_target {
                                                    egui::Color32::from_rgb(100, 150, 255)
                                                } else if is_diatonic {
                                                    ui.visuals().widgets.inactive.bg_fill
                                                } else {
                                                    ui.visuals().widgets.noninteractive.bg_fill
                                                };

                                                let button = egui::Button::new(label)
                                                    .min_size(egui::vec2(ui.available_width() / diatonics.len() as f32, 0.0))
                                                    .fill(button_color);
                                                let response = ui.add(button);

                                                if response.is_pointer_button_down_on() {
                                                    if egui_ctx.input(|i| i.modifiers.ctrl) {
                                                        state.inversion_chord = Some(chord_id.clone());
                                                        let _ = sender
                                                            .send(MidiMessage::SetInversionChord(chord_id.clone()));
                                                    } else if state.playing_chord.as_ref() != Some(&chord_id) {
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
                        }
                        ViewMode::KeyMapping => {
                            ui.heading("Key Mapping");
                            ui.add_space(10.0);

                            let keys_to_map = [
                                egui::Key::Z,
                                egui::Key::X,
                                egui::Key::C,
                                egui::Key::V,
                                egui::Key::B,
                                egui::Key::N,
                                egui::Key::M,
                            ];

                            if let Some(key_to_map) = state.key_to_map {
                                ui.horizontal(|ui| {
                                    ui.label(format!("Select a chord for key {:?}", key_to_map));
                                    if ui.button("Cancel").clicked() {
                                        state.key_to_map = None;
                                    }
                                });

                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    egui::Grid::new("key_map_chord_selection_grid").show(
                                        ui,
                                        |ui| {
                                            ui.label("");
                                            for d in &diatonics {
                                                ui.strong(&d.degree);
                                            }
                                            ui.end_row();

                                            for &(type_key, suffix) in &grid_rows {
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
                                                        let label =
                                                            format!("{}{}", root_note, suffix);
                                                        let button = egui::Button::new(label)
                                                            .min_size(egui::vec2(
                                                                ui.available_width()
                                                                    / diatonics.len() as f32,
                                                                0.0,
                                                            ));

                                                        if ui.add(button).clicked() {
                                                            let _ = sender.send(
                                                                MidiMessage::UpdateKeyMapping(
                                                                    key_to_map,
                                                                    chord_id.clone(),
                                                                ),
                                                            );
                                                            state.key_mappings.insert(
                                                                key_to_map,
                                                                chord_id.clone(),
                                                            );
                                                            state.key_to_map = None;
                                                        }
                                                    } else {
                                                        ui.label("");
                                                    }
                                                }
                                                ui.end_row();
                                            }
                                        },
                                    );
                                });
                            } else {
                                egui::Grid::new("key_mapping_grid")
                                    .num_columns(3)
                                    .spacing([40.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        for key in keys_to_map.iter() {
                                            ui.label(format!("{:?}", key));

                                            let mapped_chord_str = state
                                                .key_mappings
                                                .get(key)
                                                .map(|c| format!("{}{}", c.root_note, c.chord_type))
                                                .unwrap_or_else(|| "None".to_string());
                                            ui.label(mapped_chord_str);

                                            if ui.button("Map").clicked() {
                                                state.key_to_map = Some(*key);
                                            }
                                            ui.end_row();
                                        }
                                    });
                            }
                        }
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

                    let current_inversion = self.state.inversion_map.get(&chord_id).copied().unwrap_or(0);
                    if let Some(voicing) = self
                        .chord_table
                        .get(&chord_id.root_note)
                        .and_then(|v| v.get(&chord_id.chord_type))
                    {
                        let num_inversions = voicing.inversions.len();
                        if num_inversions > 0 {
                            let inversion_idx = current_inversion as usize % num_inversions;
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
                MidiMessage::UpdateInversion(chord_id, inversion) => {
                    self.state.inversion_map.insert(chord_id, inversion);
                }
                MidiMessage::UpdateScale(scale) => {
                    let parts: Vec<&str> = scale.split_whitespace().collect();
                    if parts.len() == 2 {
                        self.state.root_note = parts[0].to_string();
                        self.state.scale_type = parts[1].to_string();
                        self.state.key_mappings =
                            generate_default_key_mappings(&self.scale_map, scale);
                    }
                }
                MidiMessage::UpdateKeyMapping(key, chord_id) => {
                    self.state.key_mappings.insert(key, chord_id);
                }
                MidiMessage::KeyChordOn(key) => {
                    if let Some(chord_id) = self.state.key_mappings.get(&key).cloned() {
                        for note in self.active_notes.drain(..) {
                            context.send_event(NoteEvent::NoteOff {
                                timing: 0,
                                voice_id: None,
                                channel: 0,
                                note,
                                velocity: 0.0,
                            });
                        }

                        let current_inversion =
                            self.state.inversion_map.get(&chord_id).copied().unwrap_or(0);
                        if let Some(voicing) = self
                            .chord_table
                            .get(&chord_id.root_note)
                            .and_then(|v| v.get(&chord_id.chord_type))
                        {
                            let num_inversions = voicing.inversions.len();
                            if num_inversions > 0 {
                                let inversion_idx = current_inversion as usize % num_inversions;
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
                        self.state.playing_key = Some(key);
                    }
                }
                MidiMessage::KeyChordOff(_key) => {
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
                    self.state.playing_key = None;
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
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(PerfectChords);
nih_export_vst3!(PerfectChords);