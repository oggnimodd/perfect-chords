use nih_plug::prelude::*;
use perfect_chords::PerfectChords;

fn main() {
    nih_export_standalone::<PerfectChords>();
}
