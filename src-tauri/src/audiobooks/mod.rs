pub mod ai_chapters;
pub mod audio;
pub mod cast;
pub mod chapters;
pub mod dsp;
pub mod engine;
pub mod import;
pub mod import_audio;
pub mod jobs;
pub mod karaoke;
pub mod listener;
pub mod m4b;
pub mod manifest;
pub mod narration;
pub mod player;
pub mod queue;
pub mod scope;
pub mod voice_marks;

pub use ai_chapters::{
    AudioChapterDetection, AudioImportMode, AudioImportOptions, detect as detect_audio_chapters,
};
pub use cast::{CastAssignment, CastInput, CastMember, CastResolver, CastVoice};
pub use chapters::{ChapterDetection, ChapterSource, DetectedChapter};
pub use dsp::{
    AudioTreatment, AudioWaveform, DEFAULT_WAVEFORM_BUCKETS, EqPreset, MAX_WAVEFORM_BUCKETS,
    MIN_WAVEFORM_BUCKETS, WaveformBuilder,
};
pub use engine::{
    AudiobookProgressEvent, audio_path, emit_progress, generate, read_manifest, regenerate,
    resolve_sections,
};
pub use import::{import, transcribe_existing};
pub use import_audio::{import as import_audio, reanalyze as reanalyze_audio_chapters};
pub use jobs::{ActiveGenerationStatus, AudiobookJobManager, GenerationControl};
pub use karaoke::{
    ChapterKaraoke, KaraokeLine, KaraokePrecision, KaraokeWord, lines_from_text, word_weight,
};
pub use listener::{Utterance, VoiceMarkerListener};
pub use manifest::{AudiobookManifest, ManifestChapter, ManifestVoice};
pub use narration::{MAX_SPEED, MIN_SPEED, NarrationProfile, NarrationSettings, NarrationStyle};
pub use player::{AudiobookPlayback, PlaybackSnapshot};
pub use scope::{AudiobookScope, SourceSection};
pub use voice_marks::{
    SegmenterConfig, UtteranceSegmenter, VoiceCommandMatch, VoiceMarkerAnchor, marker_anchor,
    match_voice_command, quote_offsets,
};
