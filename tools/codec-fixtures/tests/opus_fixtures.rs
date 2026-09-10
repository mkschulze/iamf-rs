//! Explicit, excluded Opus fixture generation and independent verification.
//! Root tests replay only the generated raw packets and manifest.
mod support;

use opus::{Application, Bitrate, Channels, Encoder};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

const FRAME_SAMPLES: usize = 960;
const CHANNELS: usize = 2;
const PACKETS: usize = 2;
const END_TRIM: usize = 1;
const MAX_PACKET_BYTES: usize = 4_000;

static TEMP_DIR_SUFFIX: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
struct Corpus {
    lookahead: usize,
    source_frames: usize,
    packets: Vec<Vec<u8>>,
    source: Vec<i16>,
    expected: Vec<i16>,
}

#[test]
#[ignore = "explicit codec fixture command only"]
fn generate_opus_corpus() {
    let dir = support::fixture_dir("CODEC_FIXTURE_OUTPUT");
    let corpus = generate();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("source.s16le"), pcm_bytes(&corpus.source)).unwrap();
    std::fs::write(dir.join("expected.s16le"), pcm_bytes(&corpus.expected)).unwrap();
    for (index, packet) in corpus.packets.iter().enumerate() {
        std::fs::write(dir.join(format!("packet-{index:03}.bin")), packet).unwrap();
    }
    let names = artifact_names(corpus.packets.len());
    let mut manifest = format!(
        "# Exact raw Opus fixture\n\n\
opus = 0.4.0\nopusic-sys = 0.7.5\nlibopus = {}\nsha2 = 0.10.9\n\
sample_rate = 48000\nchannels = 2\nframe_samples = 960\n\
application = audio\nbitrate = 128000\nvbr = false\ncomplexity = 10\nforce_channels = stereo\nmax_bandwidth = fullband\nsignal = music\ndtx = false\ninband_fec = false\npacket_loss_perc = 0\nlsb_depth = 16\n\
L = {}\nS = {}\nP = {}\nE = {}\npadded = {}\n\
packet_order = {}\n\
Source: deterministic interleaved L/R signed 16-bit little-endian PCM, zero-padded after S.\n\
sample(channel, index) = ((channel * 104729 + index * 1299709) % 24577) - 12288.\n\
All source samples have absolute value <= 12288 (-8.5 dBFS), below the limiter threshold.\n\
The encoder is configured before querying L. S is selected after that query as\n\
two 960-frame packets minus L and one trailing padding frame; P is ceil((S + L)/960), and E is one.\n\
Each packet is raw Opus data only: no Ogg container and no OpusHead.\n\
expected.s16le is independently decoded by a fresh decoder from all P packets,\n\
then trimmed by L leading and E trailing stereo frames; source.s16le is provenance only.\n\n\
Run from the repository root (relative paths resolve there):\n\n\
```sh\n\
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact generate_opus_corpus\n\
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact verify_opus_corpus\n\
```\n\n",
        opus::version(),
        corpus.lookahead,
        corpus.source_frames,
        corpus.packets.len(),
        END_TRIM,
        corpus.packets.len() * FRAME_SAMPLES,
        corpus
            .packets
            .iter()
            .enumerate()
            .map(|(index, _)| format!("packet-{index:03}.bin"))
            .collect::<Vec<_>>()
            .join(",")
    );
    for name in &names {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        if name.starts_with("packet-") {
            manifest.push_str(&format!("packet_len.{name} = {}\n", bytes.len()));
        }
        manifest.push_str(&format!("sha256.{name} = {}\n", support::digest(&bytes)));
    }
    std::fs::write(dir.join("MANIFEST.md"), manifest).unwrap();
    verify(&dir);
}

#[test]
#[ignore = "explicit codec fixture command only"]
fn verify_opus_corpus() {
    verify(&support::fixture_dir("CODEC_FIXTURE_INPUT"));
}

#[test]
fn verify_rejects_self_consistent_manifest_shifted_one_frame_lookahead() {
    let dir = temporary_fixture_dir();
    write_shifted_lookahead_fixture(&dir);
    let result = std::panic::catch_unwind(|| verify(&dir));
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        result.is_err(),
        "a shifted manifest L/S and matching shifted provenance/output digests must be rejected"
    );
}

fn configure_encoder() -> Encoder {
    let mut encoder = Encoder::new(48_000, Channels::Stereo, Application::Audio).unwrap();
    encoder.set_bitrate(Bitrate::Bits(128_000)).unwrap();
    encoder.set_vbr(false).unwrap();
    encoder.set_complexity(10).unwrap();
    encoder.set_force_channels(Some(Channels::Stereo)).unwrap();
    encoder
        .set_max_bandwidth(opus::Bandwidth::Fullband)
        .unwrap();
    encoder.set_signal(opus::Signal::Music).unwrap();
    encoder.set_dtx(false).unwrap();
    encoder.set_inband_fec(false).unwrap();
    encoder.set_packet_loss_perc(0).unwrap();
    encoder.set_lsb_depth(16).unwrap();
    encoder
}

fn generate() -> Corpus {
    let mut encoder = configure_encoder();
    let lookahead = configured_lookahead(&mut encoder);
    assert!(lookahead > 0);
    let source_frames = PACKETS * FRAME_SAMPLES - lookahead - END_TRIM;
    assert!(source_frames > 0);
    assert_eq!((source_frames + lookahead).div_ceil(FRAME_SAMPLES), PACKETS);
    assert_ne!(END_TRIM, lookahead);

    let source = signal(source_frames);
    let mut padded = source.clone();
    padded.resize(PACKETS * FRAME_SAMPLES * CHANNELS, 0);
    assert_eq!(padded.len(), PACKETS * FRAME_SAMPLES * CHANNELS);
    let packets = padded
        .chunks_exact(FRAME_SAMPLES * CHANNELS)
        .map(|frame| encoder.encode_vec(frame, MAX_PACKET_BYTES).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(packets.len(), PACKETS);
    assert!(packets.iter().all(|packet| !packet.is_empty()));
    assert!(
        packets
            .iter()
            .all(|packet| !packet.windows(4).any(|window| window == b"OggS"))
    );
    assert!(
        packets
            .iter()
            .all(|packet| !packet.windows(8).any(|window| window == b"OpusHead"))
    );
    let expected = decode_and_trim(&packets, lookahead, END_TRIM);
    assert_eq!(expected.len(), source_frames * CHANNELS);
    Corpus {
        lookahead,
        source_frames,
        packets,
        source,
        expected,
    }
}

fn configured_lookahead(encoder: &mut Encoder) -> usize {
    usize::try_from(encoder.get_lookahead().unwrap()).unwrap()
}

fn signal(frames: usize) -> Vec<i16> {
    let source = (0..frames)
        .flat_map(|index| {
            (0..CHANNELS).map(move |channel| {
                (((channel as i64) * 104_729 + (index as i64) * 1_299_709) % 24_577 - 12_288) as i16
            })
        })
        .collect::<Vec<_>>();
    assert!(source.iter().all(|sample| sample.unsigned_abs() <= 12_288));
    assert!(source.chunks_exact(CHANNELS).all(|pair| pair[0] != pair[1]));
    source
}

fn decode_and_trim(packets: &[Vec<u8>], lookahead: usize, end_trim: usize) -> Vec<i16> {
    let mut decoder = opus::Decoder::new(48_000, Channels::Stereo).unwrap();
    let mut decoded = Vec::with_capacity(packets.len() * FRAME_SAMPLES * CHANNELS);
    for packet in packets {
        let mut output = [0_i16; FRAME_SAMPLES * CHANNELS];
        let frames = decoder.decode(packet, &mut output, false).unwrap();
        assert_eq!(
            frames, FRAME_SAMPLES,
            "every packet must decode to 960 frames"
        );
        decoded.extend_from_slice(&output[..frames * CHANNELS]);
    }
    assert_eq!(decoded.len(), packets.len() * FRAME_SAMPLES * CHANNELS);
    let start = lookahead * CHANNELS;
    let end = decoded.len() - end_trim * CHANNELS;
    decoded[start..end].to_vec()
}

fn artifact_names(packets: usize) -> Vec<String> {
    let mut names = vec!["source.s16le".to_owned(), "expected.s16le".to_owned()];
    names.extend((0..packets).map(|index| format!("packet-{index:03}.bin")));
    names
}

fn temporary_fixture_dir() -> std::path::PathBuf {
    let suffix = TEMP_DIR_SUFFIX.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "iamf-opus-fixture-lookahead-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    dir
}

fn write_shifted_lookahead_fixture(dir: &Path) {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/codecs/opus");
    let fields = support::manifest(&corpus);
    let lookahead: usize = fields["L"].parse().unwrap();
    let source_frames: usize = fields["S"].parse().unwrap();
    for name in ["packet-000.bin", "packet-001.bin"] {
        std::fs::copy(corpus.join(name), dir.join(name)).unwrap();
    }
    let mut source = std::fs::read(corpus.join("source.s16le")).unwrap();
    source.truncate(source.len() - CHANNELS * 2);
    std::fs::write(dir.join("source.s16le"), &source).unwrap();
    let expected = std::fs::read(corpus.join("expected.s16le")).unwrap();
    let expected = expected[CHANNELS * 2..].to_vec();
    std::fs::write(dir.join("expected.s16le"), &expected).unwrap();
    let manifest = std::fs::read_to_string(corpus.join("MANIFEST.md")).unwrap();
    let manifest = manifest
        .lines()
        .map(|line| match line {
            line if line.starts_with("L = ") => format!("L = {}", lookahead + 1),
            line if line.starts_with("S = ") => format!("S = {}", source_frames - 1),
            line if line.starts_with("sha256.source.s16le = ") => {
                format!("sha256.source.s16le = {}", support::digest(&source))
            }
            line if line.starts_with("sha256.expected.s16le = ") => {
                format!("sha256.expected.s16le = {}", support::digest(&expected))
            }
            line => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.join("MANIFEST.md"), format!("{manifest}\n")).unwrap();
}

fn pcm_bytes(samples: &[i16]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

fn verify(dir: &Path) {
    let fields = support::manifest(dir);
    for (key, value) in [
        ("opus", "0.4.0"),
        ("opusic-sys", "0.7.5"),
        ("sha2", "0.10.9"),
        ("sample_rate", "48000"),
        ("channels", "2"),
        ("frame_samples", "960"),
        ("application", "audio"),
        ("bitrate", "128000"),
        ("vbr", "false"),
        ("complexity", "10"),
        ("force_channels", "stereo"),
        ("max_bandwidth", "fullband"),
        ("signal", "music"),
        ("dtx", "false"),
        ("inband_fec", "false"),
        ("packet_loss_perc", "0"),
        ("lsb_depth", "16"),
    ] {
        assert_eq!(fields.get(key).map(String::as_str), Some(value), "{key}");
    }
    let number = |key: &str| -> usize { fields[key].parse().unwrap() };
    let lookahead = number("L");
    let source_frames = number("S");
    let packets = number("P");
    let end_trim = number("E");
    let mut encoder = configure_encoder();
    assert_eq!(
        lookahead,
        configured_lookahead(&mut encoder),
        "manifest L must equal configured encoder lookahead"
    );
    assert!(lookahead > 0);
    assert_eq!(packets, (source_frames + lookahead).div_ceil(FRAME_SAMPLES));
    assert!(packets >= 2);
    assert_eq!(
        end_trim,
        packets * FRAME_SAMPLES - lookahead - source_frames
    );
    assert!(end_trim > 0 && end_trim < FRAME_SAMPLES);
    assert_ne!(end_trim, lookahead);

    let canonical = (0..packets)
        .map(|index| format!("packet-{index:03}.bin"))
        .collect::<Vec<_>>();
    assert_eq!(
        fields["packet_order"].split(',').collect::<Vec<_>>(),
        canonical.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let mut actual = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected_names = vec!["MANIFEST.md".to_owned()];
    expected_names.extend(artifact_names(packets));
    expected_names.sort();
    assert_eq!(actual, expected_names);

    let source = std::fs::read(dir.join("source.s16le")).unwrap();
    assert_eq!(source.len(), source_frames * CHANNELS * 2);
    assert_eq!(
        fields.get("sha256.source.s16le"),
        Some(&support::digest(&source))
    );
    let source_pcm = support::pcm_s16le(&source);
    assert!(
        source_pcm
            .iter()
            .all(|sample| sample.unsigned_abs() <= 12_288)
    );
    assert!(
        source_pcm
            .chunks_exact(CHANNELS)
            .all(|pair| pair[0] != pair[1])
    );

    let packets = canonical
        .iter()
        .map(|name| {
            let packet = std::fs::read(dir.join(name)).unwrap();
            assert!(!packet.is_empty(), "{name}");
            assert!(!packet.windows(4).any(|window| window == b"OggS"), "{name}");
            assert!(
                !packet.windows(8).any(|window| window == b"OpusHead"),
                "{name}"
            );
            assert_eq!(
                fields.get(&format!("packet_len.{name}")),
                Some(&packet.len().to_string())
            );
            assert_eq!(
                fields.get(&format!("sha256.{name}")),
                Some(&support::digest(&packet))
            );
            packet
        })
        .collect::<Vec<_>>();
    let fresh = decode_and_trim(&packets, lookahead, end_trim);
    assert_eq!(fresh.len(), source_frames * CHANNELS);
    let expected = std::fs::read(dir.join("expected.s16le")).unwrap();
    assert_eq!(
        fields.get("sha256.expected.s16le"),
        Some(&support::digest(&expected))
    );
    assert_eq!(
        pcm_bytes(&fresh),
        expected,
        "fresh decoder output must be exact"
    );
}
