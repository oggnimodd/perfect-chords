use crossbeam_channel::{Receiver, Sender};
use nih_plug::prelude::*;
use nih_plug_egui::{EguiState, create_egui_editor, egui};
use std::sync::Arc;

enum MidiMessage {
    ChordOn,
    ChordOff,
}

#[derive(Default)]
struct GuiState {
    is_playing: bool,
}

pub struct PerfectChords {
    params: Arc<PerfectChordsParams>,
    midi_sender: Sender<MidiMessage>,
    midi_receiver: Receiver<MidiMessage>,
    active_notes: Vec<u8>,
}

#[derive(Params)]
pub struct PerfectChordsParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for PerfectChords {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self {
            params: Arc::new(PerfectChordsParams::default()),
            midi_sender: sender,
            midi_receiver: receiver,
            active_notes: Vec::new(),
        }
    }
}

impl Default for PerfectChordsParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(300, 180),
        }
    }
}

impl Plugin for PerfectChords {
    const NAME: &'static str = "Perfect Notes (Standalone)";
    const VENDOR: &'static str = "";
    const URL: &'static str = "https://example.com";
    const EMAIL: &'static str = "user@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
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

        create_egui_editor(
            self.params.editor_state.clone(),
            GuiState::default(),
            |_, _| {},
            move |egui_ctx, _setter, state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.heading("Standalone MIDI Player");
                    ui.add_space(10.0);

                    let (rect, response) = ui.allocate_at_least(
                        egui::vec2(ui.available_width(), 40.0),
                        egui::Sense::drag(),
                    );

                    let visuals = ui.style().interact_selectable(&response, state.is_playing);

                    ui.painter()
                        .rect_filled(rect, visuals.corner_radius, visuals.bg_fill);

                    ui.painter().rect_stroke(
                        rect,
                        visuals.corner_radius,
                        (visuals.bg_stroke.width, visuals.bg_stroke.color),
                        egui::StrokeKind::Inside,
                    );

                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Play C-Major Chord",
                        egui::FontId::default(),
                        visuals.fg_stroke.color,
                    );

                    if response.drag_started() {
                        if !state.is_playing {
                            sender.send(MidiMessage::ChordOn).unwrap();
                            state.is_playing = true;
                        }
                    }

                    if response.drag_stopped() {
                        if state.is_playing {
                            sender.send(MidiMessage::ChordOff).unwrap();
                            state.is_playing = false;
                        }
                    }

                    ui.add_space(20.0);
                    ui.separator();
                    ui.label("DEBUG INFO:");
                    ui.label(format!("Currently Playing: {}", state.is_playing));
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
                MidiMessage::ChordOn => {
                    let notes: [u8; 3] = [60, 64, 67];
                    for &note in &notes {
                        context.send_event(NoteEvent::NoteOn {
                            timing: 0,
                            voice_id: None,
                            channel: 0,
                            note,
                            velocity: 0.8,
                        });
                        self.active_notes.push(note);
                    }
                }
                MidiMessage::ChordOff => {
                    for &note in &self.active_notes {
                        context.send_event(NoteEvent::NoteOff {
                            timing: 0,
                            voice_id: None,
                            channel: 0,
                            note,
                            velocity: 0.0,
                        });
                    }
                    self.active_notes.clear();
                }
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for PerfectChords {
    const CLAP_ID: &'static str = "com.perfect-notes-standalone";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A standalone MIDI player.");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect, ClapFeature::Utility];
}

impl Vst3Plugin for PerfectChords {
    const VST3_CLASS_ID: [u8; 16] = *b"PerfChord2Plugin";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(PerfectChords);
nih_export_vst3!(PerfectChords);
