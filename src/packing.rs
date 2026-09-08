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
    pub fn for_layout(_layout: LoudspeakerLayout) -> Result<Self> {
        // STUB(GREEN): the ordering rule lands with the wire behaviour.
        Ok(Self {
            substreams: Vec::new(),
        })
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
        self.substreams
            .iter()
            .fold(0_usize, |total, spec| {
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
    _plan: &SubstreamPlan,
    _interleaved: &[u8],
    _channel_count: usize,
    _bytes_per_sample: usize,
) -> Result<Vec<Vec<u8>>> {
    // STUB(GREEN): regroups nothing.
    let _ = (
        ObuType::AudioFrame,
        obu_type_for(0),
        Error::new(ErrorKind::UnsupportedLayout, Location::Unlocated),
    );
    Ok(Vec::new())
}
