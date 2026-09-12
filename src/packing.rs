//! TIME-02 — BCG channel→substream packing: which channels go into which
//! Audio Frame, and in what order inside it.
//!
//! `PROJECT.md` names this the highest silent-failure risk in Phase 1. A wrong
//! order produces a **clean decode with scrambled channels**: no error, no
//! crash, correct byte count, nothing downstream that can see it. The tests in
//! `tests/packing.rs` are the only detector, which is why they assert the
//! answer three independent ways rather than one.
//!
//! # This is a permutation and a regroup, not DSP
//!
//! Nothing here performs arithmetic on a sample value. The only arithmetic is
//! on **indices and byte lengths**; samples are moved as opaque byte slices and
//! never scaled, summed, converted or rounded. That is what keeps this file
//! inside the scope boundary `PROJECT.md` draws around Parallax's `D-40`, and
//! it is why GUARD-11's no-DSP clippy guard stays green through it with no
//! `#[allow]` — the D-21 escape census must still read zero here.
//!
//! # The ordering rule, quoted from `libiamf`'s own comment
//!
//! ```text
//! In ChannelGroup for Channel audio: The order conforms to following rules:
//!
//! @ Coupled Substream(s) comes first and followed by non-coupled Substream(s).
//! @ Coupled Substream(s) for surround channels comes first and followed by
//!   one(s) for top channels.
//! @ Coupled Substream(s) for front channels comes first and followed by one(s)
//!   for side, rear and back channels.
//! @ Coupled Substream(s) for side channels comes first and followed by one(s)
//!   for rear channels.
//! @ Center channel comes first and followed by LFE and followed by the other
//!   one.
//! ```
//! `libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:365-378`
//!
//! For 5.1 that resolves to `[L,R]`, `[Ls,Rs]`, `C`, `LFE` — two coupled pairs
//! then two monos. Two further derivations agree: the layout table's
//! `decoding_map = {0, 1, 4, 5, 2, 3}` over
//! `channel_layout = {L5, R5, C, LFE, SL5, SR5}`
//! (`libiamf@v1.1.0 code/src/iamf_dec/IAMF_layout.c:57-74`), and the shipped
//! `tones_256samp_5p1_pcm.iamf`'s own OBU sizes — 1024, 1024, 512, 512.
//!
//! # The channel order is CITED, not derived (D-19)
//!
//! The indices this module returns are positions in the **decoder's documented
//! output order** for the corresponding sound system:
//!
//! | Layout | Channel order |
//! |---|---|
//! | Sound System A (0+2+0) | L, R |
//! | Sound System B (0+5+0) | **L, R, C, LFE, Ls, Rs** |
//! | Sound System C (2+5+0) | L, R, C, LFE, Ls, Rs, Ltf, Rtf |
//! | Sound System D (4+5+0) | L, R, C, LFE, Ls, Rs, Ltf, Rtf, Ltr, Rtr |
//! | Sound System I (0+7+0) | L, R, C, LFE, Lss, Rss, Lrs, Rrs |
//! | Sound System J (4+7+0) | L, R, C, LFE, Lss, Rss, Lrs, Rrs, Ltf, Rtf, Ltb, Rtb |
//!
//! Two independent citations for the 5.1 row:
//! `libiamf@v1.1.0 code/src/iamf_dec/IAMF_layout.c:72-73`
//! (`.channel_layout = {IA_CH_L5, IA_CH_R5, IA_CH_C, IA_CH_LFE, IA_CH_SL5, IA_CH_SR5}`)
//! and `iamf-tools@v2.1.0 iamf/cli/testdata/README.md`, "Output WAV files"
//! (`Sound System B (0+5+0) | ITU-2051-3 | L, R, C, LFE, Ls, Rs`). A third,
//! from the encoder side:
//! `iamf-tools@v2.1.0 iamf/cli/channel_label.cc:47`
//! (`{kLayout5_1_ch, {kL5, kR5, kCentre, kLFE, kLs5, kRs5}}`).
//!
//! **There is no permutation constant anywhere in this crate, and there must
//! never be one.** The 5.1 → Sound System B render matrix in `libiamf` is the
//! 6×6 identity (`code/src/iamf_dec/m2m_rdr.c:106-108, 1222`), so multiplying
//! by exactly `1.0f` and summing one term are IEEE-754-exact and the decoder's
//! output **is** the caller's input. Comparison is therefore direct
//! sample-for-sample equality with nothing to tune. A permutation constant is a
//! knob, and the cheapest-looking fix for a failing comparison is to turn the
//! knob until it goes green — which absorbs a real packing bug into the harness
//! and ships it. No knob, no knob to turn.
//!
//! # Two different layouts, and they are not the same layout
//!
//! Inside a **coupled substream** the payload is **sample-interleaved**:
//! `L₀ R₀ L₁ R₁ …`, taken from the decoder's own read loop
//! (`libiamf@v1.1.0 code/src/iamf_dec/pcm/IAMF_pcm_decoder.c:113-131`, where
//! the coupled arm indexes `(s * 2 + lf) * sample_size_bytes`). The decoder
//! writes its *output* **planar** per channel — `fpcm[samples * ch + s]` in the
//! same loop. Those are two different layouts and conflating them is the same
//! class of error as the packing order itself.

use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::layout::LoudspeakerLayout;
use crate::obu::{ObuType, obu_type_for};

/// The channels one substream carries, as positions in the caller's
/// interleaved input in the decoder's documented output order.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstreamChannels {
    /// A coupled pair, written sample-interleaved: first, second, first, ….
    Coupled(usize, usize),
    /// A single channel.
    Mono(usize),
}

impl SubstreamChannels {
    /// How many source channels this substream consumes: 2 or 1.
    #[must_use]
    pub const fn channel_count(self) -> usize {
        match self {
            Self::Coupled(_, _) => 2,
            Self::Mono(_) => 1,
        }
    }

    /// Whether this substream is one of the `coupled_substream_count`.
    #[must_use]
    pub const fn is_coupled(self) -> bool {
        matches!(self, Self::Coupled(_, _))
    }
}

/// One substream: the OBU type its frames are written as, and the channels it
/// carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubstreamSpec {
    /// The Audio Frame OBU type — `6 + substream_index` under TIME-01's
    /// implicit-id rule, which is what ties this module to `audio_frame.rs`.
    pub obu_type: ObuType,
    /// The source channels, in the caller's input order.
    pub channels: SubstreamChannels,
}

/// The substreams a channel layout packs into, in **bitstream order**.
///
/// `substream_count` and `coupled_substream_count` are **derived from this
/// plan** rather than taken as inputs, so the numbers a
/// `ChannelAudioLayerConfig` declares and the frames actually written come
/// from one source. Two sources for the same two numbers is two numbers that
/// eventually differ, and `libiamf` treats a frame count that disagrees with
/// `substream_count` as a hard error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstreamPlan {
    substreams: Vec<SubstreamSpec>,
}

impl SubstreamPlan {
    /// The packing plan for a channel layout.
    ///
    /// Phase 1 implements the three layouts research closed — mono, stereo and
    /// 5.1 — and returns [`ErrorKind::UnsupportedLayout`] for the rest. That is
    /// deliberate: inventing an ordering the reference never states would
    /// produce exactly the silent, clean-decode-wrong-channels failure this
    /// module exists to prevent, and a typed error is visible where a guess is
    /// not.
    pub fn for_layout(layout: LoudspeakerLayout) -> Result<Self> {
        // Each entry is the source channel(s) for one substream, in the
        // bitstream order the ordering rule fixes: coupled first, then mono;
        // and within the monos, centre before LFE.
        let groups: &[SubstreamChannels] = match layout {
            // Sound System A order is `L, R`; one coupled pair, no monos.
            // Matches test_000003's Audio Element (substream_count 1,
            // coupled_substream_count 1).
            LoudspeakerLayout::Stereo | LoudspeakerLayout::Binaural => {
                &[SubstreamChannels::Coupled(0, 1)]
            }
            // A single centre channel; nothing to couple it with.
            LoudspeakerLayout::Mono => &[SubstreamChannels::Mono(0)],
            // Sound System B order is `L, R, C, LFE, Ls, Rs`. Coupled front
            // pair first, then the coupled surround pair, then centre, then
            // LFE — which is why the source indices are 0,1 / 4,5 / 2 / 3 and
            // NOT 0,1 / 2,3 / 4 / 5. Packing order and presentation order are
            // different orders; that difference is the whole of TIME-02.
            LoudspeakerLayout::Ch5_1 => &[
                SubstreamChannels::Coupled(0, 1),
                SubstreamChannels::Coupled(4, 5),
                SubstreamChannels::Mono(2),
                SubstreamChannels::Mono(3),
            ],
            // Everything else: a typed error, not a guess. See the doc comment.
            _ => {
                return Err(Error::new(
                    ErrorKind::UnsupportedLayout,
                    Location::Field("loudspeaker_layout"),
                ));
            }
        };

        let mut substreams = Vec::with_capacity(groups.len());
        for (index, channels) in groups.iter().enumerate() {
            let id = u32::try_from(index).map_err(|_| {
                Error::new(
                    ErrorKind::UnsupportedLayout,
                    Location::Field("substream_index"),
                )
            })?;
            substreams.push(SubstreamSpec {
                // TIME-01: substream `n` is written as OBU type `6 + n`, with
                // no explicit id in the payload. The type comes from
                // `audio_frame.rs`'s encoder so the two cannot disagree.
                obu_type: obu_type_for(id),
                channels: *channels,
            });
        }
        Ok(Self { substreams })
    }

    /// The substreams, in bitstream order.
    #[must_use]
    pub fn substreams(&self) -> &[SubstreamSpec] {
        &self.substreams
    }

    /// `substream_count`, derived.
    #[must_use]
    pub fn substream_count(&self) -> u8 {
        u8::try_from(self.substreams.len()).unwrap_or(u8::MAX)
    }

    /// `coupled_substream_count`, derived.
    #[must_use]
    pub fn coupled_substream_count(&self) -> u8 {
        u8::try_from(
            self.substreams
                .iter()
                .filter(|spec| spec.channels.is_coupled())
                .count(),
        )
        .unwrap_or(u8::MAX)
    }

    /// The number of source channels this plan consumes, derived.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.substreams.iter().fold(0_usize, |total, spec| {
            total.saturating_add(spec.channels.channel_count())
        })
    }
}

/// Regroup interleaved PCM into one payload per substream.
///
/// `interleaved` is `channel_count` channels of `bytes_per_sample`-byte samples
/// in the decoder's documented output order. The return value is one byte
/// buffer per substream, in the plan's order, ready to become an Audio Frame
/// payload.
///
/// No arithmetic is performed on any sample value: bytes are copied from one
/// slice to another and nothing else.
pub fn pack_channels_to_substreams(
    plan: &SubstreamPlan,
    interleaved: &[u8],
    channel_count: usize,
    bytes_per_sample: usize,
) -> Result<Vec<Vec<u8>>> {
    let mismatch = || {
        Error::new(
            ErrorKind::ChannelCountMismatch,
            Location::Field("interleaved"),
        )
    };

    if channel_count != plan.channel_count() || channel_count == 0 || bytes_per_sample == 0 {
        return Err(mismatch());
    }
    // One interleaved sample frame, in bytes. Checked, because both factors
    // are caller-supplied.
    let frame_bytes = channel_count
        .checked_mul(bytes_per_sample)
        .ok_or_else(mismatch)?;
    // `frame_bytes` is non-zero (both factors were checked above), but the
    // division is still `checked_rem`/`checked_div` rather than `%` and `/`:
    // GUARD-03 denies bare arithmetic precisely because "non-zero by argument"
    // stops being true when the argument is edited.
    if interleaved.len().checked_rem(frame_bytes) != Some(0) {
        // A partial trailing frame would be packed short or long and the
        // decoder would mis-frame it, so it is refused here rather than
        // silently truncated.
        return Err(mismatch());
    }
    let samples = interleaved
        .len()
        .checked_div(frame_bytes)
        .ok_or_else(mismatch)?;

    let mut packed = Vec::with_capacity(plan.substreams.len());
    for spec in &plan.substreams {
        let sources: &[usize] = match spec.channels {
            SubstreamChannels::Coupled(first, second) => &[first, second],
            SubstreamChannels::Mono(only) => &[only],
        };
        let payload_len = samples
            .checked_mul(sources.len())
            .and_then(|total| total.checked_mul(bytes_per_sample))
            .ok_or_else(mismatch)?;
        let mut payload = Vec::with_capacity(payload_len);

        for sample in 0..samples {
            let frame_start = sample.checked_mul(frame_bytes).ok_or_else(mismatch)?;
            // Sample-interleaved inside a coupled substream: for each sample
            // index, the first channel then the second. Not planar.
            for source in sources {
                if *source >= channel_count {
                    return Err(mismatch());
                }
                let start = source
                    .checked_mul(bytes_per_sample)
                    .and_then(|offset| offset.checked_add(frame_start))
                    .ok_or_else(mismatch)?;
                let end = start.checked_add(bytes_per_sample).ok_or_else(mismatch)?;
                // A copy of opaque bytes. No arithmetic is performed on the
                // sample value itself — see the module comment.
                let bytes = interleaved.get(start..end).ok_or_else(mismatch)?;
                payload.extend_from_slice(bytes);
            }
        }
        packed.push(payload);
    }
    Ok(packed)
}
