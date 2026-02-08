use macroquad::audio::{load_sound, play_sound, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::Vec2;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use crate::helpers::asset_path;

#[derive(Debug)]
pub enum SoundLoadError {
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
    Sound(String),
}

impl std::fmt::Display for SoundLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Yaml(err) => write!(f, "yaml error: {err}"),
            Self::Sound(err) => write!(f, "sound error: {err}"),
        }
    }
}

impl std::error::Error for SoundLoadError {}

impl From<std::io::Error> for SoundLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_yaml::Error> for SoundLoadError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Yaml(err)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SoundChannel {
    Ui,
    Sfx,
    Ambient,
    Music,
}

#[derive(Clone)]
pub struct SoundEntry {
    pub id: String,
    pub channel: SoundChannel,
    pub volume: f32,
    pub looped: bool,
    pub pitch: f32,
    pub spatial: bool,
    pub max_distance: f32,
    pub min_distance: f32,
    pub variance: f32,
}

#[derive(Clone)]
struct LoadedSound {
    entry: SoundEntry,
    sound: Sound,
}

pub struct SoundSystem {
    sounds: Vec<LoadedSound>,
    lookup: HashMap<String, usize>,
    channel_volume: HashMap<SoundChannel, f32>,
}

impl SoundSystem {
    pub fn empty() -> Self {
        let mut channel_volume = HashMap::new();
        channel_volume.insert(SoundChannel::Ui, 1.0);
        channel_volume.insert(SoundChannel::Sfx, 1.0);
        channel_volume.insert(SoundChannel::Ambient, 1.0);
        channel_volume.insert(SoundChannel::Music, 1.0);
        Self {
            sounds: Vec::new(),
            lookup: HashMap::new(),
            channel_volume,
        }
    }

    pub async fn load_from(dir: impl AsRef<Path>) -> Result<Self, SoundLoadError> {
        if cfg!(target_arch = "wasm32") {
            return Ok(Self::empty());
        }
        let dir = dir.as_ref();
        let mut sounds = Vec::new();
        let mut lookup = HashMap::new();

        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if !is_yaml(&path) {
                    continue;
                }
                let raw: SoundFile = serde_yaml::from_str(&std::fs::read_to_string(&path)?)?;
                let sound = load_sound(&asset_path(&raw.path))
                    .await
                    .map_err(|err| SoundLoadError::Sound(err.to_string()))?;

                let entry = SoundEntry {
                    id: raw.id.clone(),
                    channel: raw.channel.unwrap_or(SoundChannel::Sfx),
                    volume: raw.volume.unwrap_or(1.0),
                    looped: raw.looped.unwrap_or(false),
                    pitch: raw.pitch.unwrap_or(1.0),
                    spatial: raw.spatial.unwrap_or(false),
                    max_distance: raw.max_distance.unwrap_or(600.0),
                    min_distance: raw.min_distance.unwrap_or(60.0),
                    variance: raw.variance.unwrap_or(0.0),
                };

                lookup.insert(raw.id, sounds.len());
                sounds.push(LoadedSound { entry, sound });
            }
        }

        let mut channel_volume = HashMap::new();
        channel_volume.insert(SoundChannel::Ui, 1.0);
        channel_volume.insert(SoundChannel::Sfx, 1.0);
        channel_volume.insert(SoundChannel::Ambient, 1.0);
        channel_volume.insert(SoundChannel::Music, 1.0);

        Ok(Self {
            sounds,
            lookup,
            channel_volume,
        })
    }

    pub fn set_channel_volume(&mut self, channel: SoundChannel, volume: f32) {
        self.channel_volume.insert(channel, volume.clamp(0.0, 1.0));
    }

    pub fn play(&self, id: &str) {
        if let Some(sound) = self.get(id) {
            // Interrupt any currently playing instance of the same sound.
            stop_sound(&sound.sound);
            let params = PlaySoundParams {
                looped: sound.entry.looped,
                volume: sound.entry.volume * self.channel_volume.get(&sound.entry.channel).copied().unwrap_or(1.0),
            };
            play_sound(&sound.sound, params);
        }
    }

    pub fn play_at(&self, id: &str, source: Vec2, listener: Vec2) {
        let Some(sound) = self.get(id) else {
            return;
        };
        if !sound.entry.spatial {
            self.play(id);
            return;
        }

        let dist = source.distance(listener);
        if dist > sound.entry.max_distance {
            return;
        }
        let volume = if dist <= sound.entry.min_distance {
            1.0
        } else {
            let t = ((dist - sound.entry.min_distance)
                / (sound.entry.max_distance - sound.entry.min_distance))
                .clamp(0.0, 1.0);
            1.0 - t
        };

        let pitch = if sound.entry.variance > 0.0 {
            let rand = macroquad::rand::gen_range(-sound.entry.variance, sound.entry.variance);
            (sound.entry.pitch + rand).max(0.05)
        } else {
            sound.entry.pitch
        };

        // Interrupt any currently playing instance of the same sound.
        stop_sound(&sound.sound);
        play_sound(
            &sound.sound,
            PlaySoundParams {
                looped: sound.entry.looped,
                volume: volume
                    * sound.entry.volume
                    * self.channel_volume.get(&sound.entry.channel).copied().unwrap_or(1.0),
            },
        );

        if pitch != 1.0 {
            // Macroquad doesn't expose pitch in PlaySoundParams; kept for future extension.
            let _ = pitch;
        }
    }

    pub fn stop(&self, id: &str) {
        if let Some(sound) = self.get(id) {
            stop_sound(&sound.sound);
        }
    }

    fn get(&self, id: &str) -> Option<&LoadedSound> {
        let idx = self.lookup.get(id).copied()?;
        self.sounds.get(idx)
    }
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
}

#[derive(Deserialize)]
struct SoundFile {
    id: String,
    path: String,
    #[serde(default)]
    channel: Option<SoundChannel>,
    #[serde(default)]
    volume: Option<f32>,
    #[serde(default)]
    looped: Option<bool>,
    #[serde(default)]
    pitch: Option<f32>,
    #[serde(default)]
    spatial: Option<bool>,
    #[serde(default)]
    max_distance: Option<f32>,
    #[serde(default)]
    min_distance: Option<f32>,
    #[serde(default)]
    variance: Option<f32>,
}
