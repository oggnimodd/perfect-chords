import * as fs from "node:fs";
import * as path from "node:path";

console.log("Starting chord generation...");

const NOTES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

const BASE_OCTAVE = 3;
const C0_MIDI_NUMBER = 12;
const BASE_MIDI_NOTE = C0_MIDI_NUMBER + BASE_OCTAVE * 12;

const MIDI_NOTE_MAP = new Map<string, number>(
  NOTES.map((note, i) => [note, BASE_MIDI_NOTE + i])
);

const CHORD_FORMULAS = new Map<string, number[]>([
  ["maj", [0, 4, 7]],
  ["m", [0, 3, 7]],
  ["dim", [0, 3, 6]],
  ["aug", [0, 4, 8]],

  ["sus2", [0, 2, 7]],
  ["sus4", [0, 5, 7]],
  ["5", [0, 7]],

  ["maj7", [0, 4, 7, 11]],
  ["m7", [0, 3, 7, 10]],
  ["7", [0, 4, 7, 10]],
  ["dim7", [0, 3, 6, 9]],
  ["m7b5", [0, 3, 6, 10]],

  ["6", [0, 4, 7, 9]],
  ["m6", [0, 3, 7, 9]],

  ["9", [0, 4, 7, 10, 14]],
  ["maj9", [0, 4, 7, 11, 14]],
  ["m9", [0, 3, 7, 10, 14]],

  ["flat5", [0, 4, 6]],
]);

const calculateInversions = (rootPositionNotes: number[]) => {
  const inversions: number[][] = [];
  let currentNotes = [...rootPositionNotes];

  for (let i = 0; i < rootPositionNotes.length; i++) {
    inversions.push([...currentNotes].sort((a, b) => a - b));

    const firstNote = currentNotes.shift();
    if (firstNote !== undefined) {
      currentNotes.push(firstNote + 12);
    }
  }
  return inversions;
};

const mapToObject = (map: Map<any, any>) => {
  const obj = Object.create(null);
  for (const [key, value] of map) {
    obj[key] = value instanceof Map ? mapToObject(value) : value;
  }
  return obj;
};

const generateChordData = () => {
  const allChords = new Map<string, Map<string, object>>();

  for (const rootNoteName of NOTES) {
    const baseMidiNote = MIDI_NOTE_MAP.get(rootNoteName);
    if (baseMidiNote === undefined) continue;

    const rootVariations = new Map<string, object>();

    for (const [chordType, formula] of CHORD_FORMULAS.entries()) {
      const rootPositionNotes = formula.map(
        (interval) => baseMidiNote + interval
      );

      const inversions = calculateInversions(rootPositionNotes);

      rootVariations.set(chordType, {
        root: rootPositionNotes,
        inversions: inversions,
      });
    }
    allChords.set(rootNoteName, rootVariations);
  }

  return allChords;
};

const main = () => {
  const generatedData = generateChordData();
  const finalDataObject = mapToObject(generatedData);

  const outputPath = path.resolve(__dirname, "chords.json");
  fs.writeFileSync(outputPath, JSON.stringify(finalDataObject, null, 2));

  console.log(
    `✅ Success! Chord data generated for ${generatedData.size} root notes.`
  );
  console.log(`✅ File saved to: ${outputPath}`);
};

main();
