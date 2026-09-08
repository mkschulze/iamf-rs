//! The IA Sequence writer — SEQ-01, SEQ-02 and SEQ-03.
//!
//! An IA Sequence is a descriptor prologue followed by temporal units, and
//! that is exactly the shape of the primitive here:
//! [`SequenceWriter::push_descriptors`], then
//! [`SequenceWriter::push_temporal_unit`] once per unit, then
//! [`SequenceWriter::finish`]. [`write_sequence`] is the whole-file
//! convenience wrapper over the same three calls and adds no logic of its own.
//!
//! # Why streaming is the primitive rather than a convenience
//!
//! **It is a memory ceiling, not a real-time-safety measure.** An hour of
//! 7.1.4 24-bit audio is roughly 4 GB of PCM and on the order of a hundred
//! thousand temporal units; none of it may ever need to be resident at once.
//! So `push_temporal_unit` hands its bytes to the sink and keeps nothing: the
//! writer holds its state machine, one reusable scratch buffer and the
//! parameter definitions the descriptors published, and nothing that grows
//! with the number of units.
//!
//! This crate is **offline end to end** (PROJECT.md Open Question 2, closed
//! 2026-09-08). It never runs on or near an audio callback — Parallax decodes
//! every format once at import into an f32 cache and streams the cache, never
//! the codec — so **no allocation or locking discipline applies in either
//! direction**. Allocate freely; use `Vec`. Both halves of that are written
//! down deliberately:
//!
//! - do not add real-time defensiveness (arena allocators, lock-free queues,
//!   `no_std`) — it would cost ergonomics to buy a guarantee nobody needs;
//! - do not remove the streaming shape thinking it was only ever about
//!   real-time safety — the memory ceiling is real and independent.
//!
//! # No signal processing lives here, ever
//!
//! The writer copies bytes and computes lengths. It does not resample, apply
//! gain, mix, pan, or pad a short final frame. A configuration a codec cannot
//! carry is a typed error and a loudness value out of range is a typed error;
//! neither is a thing to fix by computing. A resampler is named in PROJECT.md
//! as the single most likely first DSP breach and this module — where a
//! caller's sample rate meets a Codec Config's — is where the temptation first
//! arrives.

use std::io::Write;

use crate::bits::BitWriter;
use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::{DescriptorSet, write_descriptors};
use crate::obu::{
    AudioFrame, Obu, ObuHeader, ObuType, ParamDefinition, ParameterBlock, TemporalDelimiter,
    write_audio_frame, write_obu_with_header, write_parameter_block, write_temporal_delimiter,
};

/// One temporal unit: everything between one presentation instant and the
/// next, in bitstream order.
///
/// The three fields are written in the order they are declared, which is the
/// order an IA Sequence carries them: the optional Temporal Delimiter, then
/// this unit's Parameter Blocks, then this unit's Audio Frames — one per
/// substream, in substream order, **all carrying identical trim values**
/// (TIME-04, enforced by [`crate::obu::validate_temporal_unit`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemporalUnit {
    /// A Temporal Delimiter OBU opening this unit, when the sequence uses
    /// them. `test_000003` sets `enable_temporal_delimiters: false`, so this is
    /// `None` for the Phase 1 reproduction.
    pub temporal_delimiter: Option<TemporalDelimiter>,
    /// The Parameter Blocks that take effect at this unit.
    ///
    /// Each is written against the [`ParamDefinition`] the **descriptors**
    /// published for its `parameter_id`; the definition is never supplied
    /// again here, because two sources for one definition is two definitions
    /// that eventually disagree.
    pub parameter_blocks: Vec<Obu<ParameterBlock>>,
    /// The Audio Frames of this unit, in substream order.
    pub audio_frames: Vec<Obu<AudioFrame>>,
}

impl TemporalUnit {
    /// A unit that is nothing but its Audio Frames — no delimiter, no
    /// parameter blocks. The shape every LPCM temporal unit in `test_000003`
    /// has.
    #[must_use]
    pub fn of_frames(audio_frames: Vec<Obu<AudioFrame>>) -> Self {
        Self {
            temporal_delimiter: None,
            parameter_blocks: Vec::new(),
            audio_frames,
        }
    }
}

/// Where the writer is in the sequence.
///
/// An explicit state machine with a distinct typed error per illegal
/// transition, rather than a debug assertion: a caller driving this from
/// Parallax's export path must get a named error it can surface, never a panic
/// that only exists in a debug build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Nothing written. Only `push_descriptors` is legal.
    Fresh,
    /// The prologue is out. `push_temporal_unit` and `finish` are legal.
    DescriptorsWritten,
    /// `finish` has been called. Nothing further is legal.
    Finished,
}

/// A streaming IA Sequence writer over any [`Write`] sink.
///
/// Append-only by construction: there is no seek, no rewrite of an earlier
/// OBU, and no buffered tail. See the module documentation for why that shape
/// is the primitive.
#[derive(Debug)]
pub struct SequenceWriter<W: Write> {
    sink: W,
    state: State,
    /// **One** scratch buffer, reused for every OBU.
    ///
    /// The two-pass `obu_size` computation needs somewhere to build an OBU
    /// before its length is known. Allocating that per frame would churn once
    /// per temporal unit for the whole file; reusing one buffer keeps the
    /// allocation profile flat, which is the whole point of SEQ-02.
    scratch: BitWriter,
    /// Every `ParamDefinition` the descriptors published, so a Parameter Block
    /// can be written against the definition that governs it without the
    /// caller supplying a second copy.
    definitions: Vec<ParamDefinition>,
    /// Bytes handed to the sink so far — the position a sink error reports at.
    bytes_written: u64,
}

impl<W: Write> SequenceWriter<W> {
    /// A writer that has emitted nothing yet.
    #[must_use]
    pub const fn new(sink: W) -> Self {
        Self {
            sink,
            state: State::Fresh,
            scratch: BitWriter::new(),
            definitions: Vec::new(),
            bytes_written: 0,
        }
    }

    /// How many bytes have been handed to the sink.
    #[must_use]
    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Write the descriptor prologue: IA Sequence Header, Codec Configs
    /// ascending by id, Audio Elements ascending by id, Mix Presentations in
    /// list order.
    ///
    /// That order is [`write_descriptors`]' (DESC-08, plan 01-05) and is not
    /// re-derived here — one place decides it.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::DescriptorsAlreadyWritten`] if called twice,
    /// [`ErrorKind::SequenceFinished`] after [`finish`](Self::finish), plus
    /// whatever the descriptor writers reject.
    pub fn push_descriptors(&mut self, descriptors: &DescriptorSet) -> Result<()> {
        match self.state {
            State::Fresh => {}
            State::DescriptorsWritten => {
                return Err(Error::new(
                    ErrorKind::DescriptorsAlreadyWritten,
                    Location::OutputOffset(self.bytes_written),
                ));
            }
            State::Finished => return Err(self.finished()),
        }

        self.definitions = collect_param_definitions(descriptors);

        self.scratch.clear();
        write_descriptors(&mut self.scratch, descriptors)?;
        self.flush_scratch()?;

        self.state = State::DescriptorsWritten;
        Ok(())
    }

    /// Write one temporal unit: its Temporal Delimiter, then its Parameter
    /// Blocks, then its Audio Frames.
    ///
    /// Nothing about the unit is retained once this returns. The caller's
    /// buffers are reusable immediately.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::DescriptorsNotWritten`] before
    /// [`push_descriptors`](Self::push_descriptors),
    /// [`ErrorKind::SequenceFinished`] after [`finish`](Self::finish),
    /// [`ErrorKind::NoGoverningParamDefinition`] for a Parameter Block whose
    /// `parameter_id` the descriptors never published, plus whatever
    /// [`crate::obu::validate_temporal_unit`] rejects.
    pub fn push_temporal_unit(&mut self, unit: &TemporalUnit) -> Result<()> {
        match self.state {
            State::Fresh => {
                return Err(Error::new(
                    ErrorKind::DescriptorsNotWritten,
                    Location::OutputOffset(self.bytes_written),
                ));
            }
            State::DescriptorsWritten => {}
            State::Finished => return Err(self.finished()),
        }

        crate::obu::validate_temporal_unit(&unit.audio_frames)?;

        if let Some(delimiter) = unit.temporal_delimiter.as_ref() {
            let obu = Obu::new(ObuHeader::new(ObuType::TemporalDelimiter), *delimiter);
            self.emit_obu(&obu, |w, _header, payload| {
                write_temporal_delimiter(w, payload)
            })?;
        }

        for block in &unit.parameter_blocks {
            // Cloned rather than borrowed: `emit_obu` takes `&mut self`, and a
            // borrow into `self.definitions` would outlive that. A
            // ParamDefinition is a handful of scalars plus at most one Vec, and
            // this path is not taken at all for LPCM.
            let definition = self.definition_for(block.payload.parameter_id)?.clone();
            self.emit_obu(block, |w, _header, payload| {
                write_parameter_block(w, &definition, payload)
            })?;
        }

        for frame in &unit.audio_frames {
            self.emit_obu(frame, write_audio_frame)?;
        }

        Ok(())
    }

    /// Close the sequence and hand back the sink.
    ///
    /// A sequence with zero temporal units is legal and produces a
    /// descriptors-only file; a sequence with no descriptors produces an empty
    /// one. Neither is an error — an IA Sequence has no trailer to omit.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::SequenceFinished`] if called twice, and whatever the sink
    /// reports when flushed.
    pub fn finish(mut self) -> Result<W> {
        if self.state == State::Finished {
            return Err(self.finished());
        }
        self.state = State::Finished;
        self.sink.flush().map_err(|_| self.sink_error())?;
        Ok(self.sink)
    }

    /// The `ParamDefinition` governing `parameter_id`, or a typed error.
    fn definition_for(&self, parameter_id: u32) -> Result<&ParamDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.parameter_id == parameter_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::NoGoverningParamDefinition,
                    Location::Field("parameter_id"),
                )
            })
    }

    /// Build one whole OBU in the reused scratch buffer and hand it to the
    /// sink.
    fn emit_obu<T, F>(&mut self, obu: &Obu<T>, write_payload: F) -> Result<()>
    where
        F: FnOnce(&mut BitWriter, &ObuHeader, &T) -> Result<()>,
    {
        self.scratch.clear();
        write_obu_with_header(&mut self.scratch, obu, write_payload)?;
        self.flush_scratch()
    }

    /// Hand whatever is in the scratch buffer to the sink and account for it.
    fn flush_scratch(&mut self) -> Result<()> {
        // STUB(GREEN): the bytes are built but never reach the sink, so every
        // byte-level assertion fails while the state machine passes.
        let _ = self.scratch.as_bytes()?;
        Ok(())
    }

    /// The error a sink refusal reports, at the position it refused.
    fn sink_error(&self) -> Error {
        Error::new(ErrorKind::SinkWrite, Location::OutputOffset(self.bytes_written))
    }

    /// The error every post-`finish` call reports.
    fn finished(&self) -> Error {
        Error::new(
            ErrorKind::SequenceFinished,
            Location::OutputOffset(self.bytes_written),
        )
    }
}

/// Every `ParamDefinition` the descriptor set publishes, in descriptor order.
///
/// A `Vec`, scanned linearly, for the reason `crate::model::by_id` is: the wire
/// can carry two definitions with one `parameter_id`, and a map would make that
/// a silent overwrite. The first published wins, which is what a decoder
/// reading forward binds to.
fn collect_param_definitions(descriptors: &DescriptorSet) -> Vec<ParamDefinition> {
    let mut definitions = Vec::new();
    for element in &descriptors.audio_elements {
        for param in &element.params {
            match param {
                crate::obu::AudioElementParam::Demixing { definition, .. }
                | crate::obu::AudioElementParam::ReconGain { definition } => {
                    definitions.push(definition.clone());
                }
                // An extension carries its bytes verbatim and no modelled
                // definition, so there is nothing to publish.
                crate::obu::AudioElementParam::Extension { .. } => {}
            }
        }
    }
    for presentation in &descriptors.mix_presentations {
        for sub_mix in &presentation.sub_mixes {
            for element in &sub_mix.elements {
                definitions.push(element.element_mix_gain.definition.clone());
            }
            definitions.push(sub_mix.output_mix_gain.definition.clone());
        }
    }
    definitions
}

// ref: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc ObuSequencerBase::PickAndPlace
// NOTE: cited for the *sequence shape* — descriptors once, then temporal units
// in presentation order — not for a line-by-line translation. The reference
// interleaves its own scheduling concerns (iterative vs one-shot output,
// timestamp bookkeeping) that this crate deliberately leaves to the caller.
/// SEQ-03 — write a whole IA Sequence in one call and hand back the sink.
///
/// **Adds no logic of its own.** It constructs a [`SequenceWriter`] and calls
/// the same three methods in the same order a streaming caller would. That is
/// deliberate: a wrapper that duplicated the serialisation would be a second
/// implementation to keep in sync, and GUARD-10's double-encode determinism
/// check would then only ever exercise one of the two.
///
/// # Errors
///
/// Whatever the streaming primitive reports.
pub fn write_sequence<W, I>(sink: W, descriptors: &DescriptorSet, units: I) -> Result<W>
where
    W: Write,
    I: IntoIterator<Item = TemporalUnit>,
{
    let mut writer = SequenceWriter::new(sink);
    writer.push_descriptors(descriptors)?;
    for unit in units {
        writer.push_temporal_unit(&unit)?;
    }
    writer.finish()
}
