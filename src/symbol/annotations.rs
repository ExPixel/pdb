use std::result;

use crate::common::*;
use crate::FallibleIterator;

/// These values correspond to the BinaryAnnotationOpcode enum from the
/// cvinfo.h
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BinaryAnnotationOpcode {
    /// Link time pdb contains PADDINGs.
    ///
    /// These are represented with the 0 opcode which is in some PDB
    /// implementation called "invalid".
    Eof = 0,
    /// param : start offset
    CodeOffset = 1,
    /// param : nth separated code chunk (main code chunk == 0)
    ChangeCodeOffsetBase = 2,
    /// param : delta of offset
    ChangeCodeOffset = 3,
    /// param : length of code, default next start
    ChangeCodeLength = 4,
    /// param : fileId
    ChangeFile = 5,
    /// param : line offset (signed)
    ChangeLineOffset = 6,
    /// param : how many lines, default 1
    ChangeLineEndDelta = 7,
    /// param : either 1 (default, for statement)
    ///         or 0 (for expression)
    ChangeRangeKind = 8,
    /// param : start column number, 0 means no column info
    ChangeColumnStart = 9,
    /// param : end column number delta (signed)
    ChangeColumnEndDelta = 10,
    /// param : ((sourceDelta << 4) | CodeDelta)
    ChangeCodeOffsetAndLineOffset = 11,
    /// param : codeLength, codeOffset
    ChangeCodeLengthAndCodeOffset = 12,
    /// param : end column number
    ChangeColumnEnd = 13,
    /// A non valid value
    Invalid,
}

impl From<u32> for BinaryAnnotationOpcode {
    fn from(value: u32) -> Self {
        match value {
            0 => BinaryAnnotationOpcode::Eof,
            1 => BinaryAnnotationOpcode::CodeOffset,
            2 => BinaryAnnotationOpcode::ChangeCodeOffsetBase,
            3 => BinaryAnnotationOpcode::ChangeCodeOffset,
            4 => BinaryAnnotationOpcode::ChangeCodeLength,
            5 => BinaryAnnotationOpcode::ChangeFile,
            6 => BinaryAnnotationOpcode::ChangeLineOffset,
            7 => BinaryAnnotationOpcode::ChangeLineEndDelta,
            8 => BinaryAnnotationOpcode::ChangeRangeKind,
            9 => BinaryAnnotationOpcode::ChangeColumnStart,
            10 => BinaryAnnotationOpcode::ChangeColumnEndDelta,
            11 => BinaryAnnotationOpcode::ChangeCodeOffsetAndLineOffset,
            12 => BinaryAnnotationOpcode::ChangeCodeLengthAndCodeOffset,
            13 => BinaryAnnotationOpcode::ChangeColumnEnd,
            _ => BinaryAnnotationOpcode::Invalid,
        }
    }
}

/// Represents a parsed `BinaryAnnotation`.
///
/// Binary annotations are used by `S_INLINESITE` to encode opcodes for how to
/// evaluate the state changes for inline information.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryAnnotation {
    CodeOffset(u32),
    ChangeCodeOffsetBase(u32),
    ChangeCodeOffset(u32),
    ChangeCodeLength(u32),
    ChangeFile(FileIndex),
    ChangeLineOffset(i32),
    ChangeLineEndDelta(u32),
    ChangeRangeKind(u32),
    ChangeColumnStart(u32),
    ChangeColumnEndDelta(i32),
    ChangeCodeOffsetAndLineOffset(u32, i32),
    ChangeCodeLengthAndCodeOffset(u32, u32),
    ChangeColumnEnd(u32),
}

impl BinaryAnnotation {
    /// Does this annotation emit a line info?
    pub fn emits_line_info(self) -> bool {
        match self {
            BinaryAnnotation::ChangeCodeOffset(..) => true,
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(..) => true,
            BinaryAnnotation::ChangeCodeLengthAndCodeOffset(..) => true,
            _ => false,
        }
    }
}

/// An iterator over binary annotations used by `S_INLINESITE`.
#[derive(Clone, Debug, Default)]
pub struct BinaryAnnotationsIter<'t> {
    buffer: ParseBuffer<'t>,
}

impl<'t> BinaryAnnotationsIter<'t> {
    fn uncompress_next(&mut self) -> Result<u32> {
        let b1 = u32::from(self.buffer.parse::<u8>()?);
        if (b1 & 0x80) == 0x00 {
            let value = b1;
            return Ok(value);
        }

        let b2 = u32::from(self.buffer.parse::<u8>()?);
        if (b1 & 0xc0) == 0x80 {
            let value = (b1 & 0x3f) << 8 | b2;
            return Ok(value);
        }

        let b3 = u32::from(self.buffer.parse::<u8>()?);
        let b4 = u32::from(self.buffer.parse::<u8>()?);
        if (b1 & 0xe0) == 0xc0 {
            let value = ((b1 & 0x1f) << 24) | (b2 << 16) | (b3 << 8) | b4;
            return Ok(value);
        }

        Err(Error::InvalidCompressedAnnotation)
    }
}

/// Resembles `DecodeSignedInt32`.
fn decode_signed_operand(value: u32) -> i32 {
    if value & 1 != 0 {
        -((value >> 1) as i32)
    } else {
        (value >> 1) as i32
    }
}

impl<'t> FallibleIterator for BinaryAnnotationsIter<'t> {
    type Item = BinaryAnnotation;
    type Error = Error;

    fn next(&mut self) -> result::Result<Option<Self::Item>, Self::Error> {
        if self.buffer.is_empty() {
            return Ok(None);
        }

        let op = self.uncompress_next()?;
        let annotation = match BinaryAnnotationOpcode::from(op) {
            BinaryAnnotationOpcode::Eof => {
                // This makes the end of the stream
                self.buffer = ParseBuffer::default();
                return Ok(None);
            }
            BinaryAnnotationOpcode::CodeOffset => {
                BinaryAnnotation::CodeOffset(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeCodeOffsetBase => {
                BinaryAnnotation::ChangeCodeOffsetBase(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeCodeOffset => {
                BinaryAnnotation::ChangeCodeOffset(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeCodeLength => {
                BinaryAnnotation::ChangeCodeLength(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeFile => {
                BinaryAnnotation::ChangeFile(FileIndex(self.uncompress_next()?))
            }
            BinaryAnnotationOpcode::ChangeLineOffset => {
                BinaryAnnotation::ChangeLineOffset(decode_signed_operand(self.uncompress_next()?))
            }
            BinaryAnnotationOpcode::ChangeLineEndDelta => {
                BinaryAnnotation::ChangeLineEndDelta(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeRangeKind => {
                BinaryAnnotation::ChangeRangeKind(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeColumnStart => {
                BinaryAnnotation::ChangeColumnStart(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::ChangeColumnEndDelta => BinaryAnnotation::ChangeColumnEndDelta(
                decode_signed_operand(self.uncompress_next()?),
            ),
            BinaryAnnotationOpcode::ChangeCodeOffsetAndLineOffset => {
                let operand = self.uncompress_next()?;
                BinaryAnnotation::ChangeCodeOffsetAndLineOffset(
                    operand & 0xf,
                    decode_signed_operand(operand >> 4),
                )
            }
            BinaryAnnotationOpcode::ChangeCodeLengthAndCodeOffset => {
                BinaryAnnotation::ChangeCodeLengthAndCodeOffset(
                    self.uncompress_next()?,
                    self.uncompress_next()?,
                )
            }
            BinaryAnnotationOpcode::ChangeColumnEnd => {
                BinaryAnnotation::ChangeColumnEnd(self.uncompress_next()?)
            }
            BinaryAnnotationOpcode::Invalid => {
                return Err(Error::UnknownBinaryAnnotation(op));
            }
        };

        Ok(Some(annotation))
    }
}

/// Binary annotations of a symbol.
///
/// The binary annotation mechanism supports recording a list of annotations in an instruction
/// stream. The X64 unwind code and the DWARF standard have a similar design.
///
/// Binary annotations are primarily used as line programs for inline function calls.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BinaryAnnotations<'t> {
    data: &'t [u8],
}

impl<'t> BinaryAnnotations<'t> {
    /// Creates a new instance of binary annotations.
    pub(crate) fn new(data: &'t [u8]) -> Self {
        BinaryAnnotations { data }
    }

    /// Iterates through binary annotations.
    pub fn iter(&self) -> BinaryAnnotationsIter<'t> {
        BinaryAnnotationsIter {
            buffer: ParseBuffer::from(self.data),
        }
    }
}

#[test]
fn test_binary_annotation_iter() {
    let inp = b"\x0b\x03\x06\n\x03\x08\x06\x06\x03-\x06\x08\x03\x07\x0br\x06\x06\x0c\x03\x07\x06\x0f\x0c\x06\x05\x00\x00";
    let annotations = BinaryAnnotations::new(inp)
        .iter()
        .collect::<Vec<_>>()
        .unwrap();

    assert_eq!(
        annotations,
        vec![
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(3, 0),
            BinaryAnnotation::ChangeLineOffset(5),
            BinaryAnnotation::ChangeCodeOffset(8),
            BinaryAnnotation::ChangeLineOffset(3),
            BinaryAnnotation::ChangeCodeOffset(45),
            BinaryAnnotation::ChangeLineOffset(4),
            BinaryAnnotation::ChangeCodeOffset(7),
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(2, -3),
            BinaryAnnotation::ChangeLineOffset(3),
            BinaryAnnotation::ChangeCodeLengthAndCodeOffset(3, 7),
            BinaryAnnotation::ChangeLineOffset(-7),
            BinaryAnnotation::ChangeCodeLengthAndCodeOffset(6, 5)
        ]
    );

    let inp = b"\x03P\x06\x0e\x03\x0c\x06\x04\x032\x06\x06\x03T\x0b#\x0b\\\x0bC\x0b/\x06\x04\x0c-\t\x03;\x06\x1d\x0c\x05\x06\x00\x00";
    let annotations = BinaryAnnotations::new(inp)
        .iter()
        .collect::<Vec<_>>()
        .unwrap();

    assert_eq!(
        annotations,
        vec![
            BinaryAnnotation::ChangeCodeOffset(80),
            BinaryAnnotation::ChangeLineOffset(7),
            BinaryAnnotation::ChangeCodeOffset(12),
            BinaryAnnotation::ChangeLineOffset(2),
            BinaryAnnotation::ChangeCodeOffset(50),
            BinaryAnnotation::ChangeLineOffset(3),
            BinaryAnnotation::ChangeCodeOffset(84),
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(3, 1),
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(12, -2),
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(3, 2),
            BinaryAnnotation::ChangeCodeOffsetAndLineOffset(15, 1),
            BinaryAnnotation::ChangeLineOffset(2),
            BinaryAnnotation::ChangeCodeLengthAndCodeOffset(45, 9),
            BinaryAnnotation::ChangeCodeOffset(59),
            BinaryAnnotation::ChangeLineOffset(-14),
            BinaryAnnotation::ChangeCodeLengthAndCodeOffset(5, 6),
        ]
    );
}
