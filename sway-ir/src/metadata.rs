///! Associated metadata attached mostly to values.
///!
///! Each value (instruction, function argument or constant) has associated metadata which helps
///! describe properties which aren't required for code generation, but help with other
///! introspective tools (e.g., the debugger) or compiler error messages.
///!
///! NOTE: At the moment the Spans contain a source string and optional path.  Any spans with no
///! path are ignored/rejected by this module.  The source string is not (de)serialised and so the
///! string is assumed to always represent the entire contents of the file path.
use std::sync::Arc;

use sway_types::span::Span;

use crate::{context::Context, error::IrError};

pub enum Metadatum {
    /// A path to a source file.
    FileLocation(Arc<std::path::PathBuf>, Arc<str>),

    /// A specific section within a source file.
    Span {
        loc_idx: MetadataIndex,
        start: usize,
        end: usize,
    },

    /// A unique token for storage operations.
    StateIndex(usize),

    /// An attribute indicating the permitted/expected storage operations with a function.
    StorageAttribute(StorageOperation),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MetadataIndex(pub generational_arena::Index);

impl MetadataIndex {
    pub fn from_span(context: &mut Context, span: &Span) -> Option<MetadataIndex> {
        // Search for an existing matching path, otherwise insert it.
        span.path().map(|path_buf| {
            let loc_idx = match context.metadata_reverse_map.get(&Arc::as_ptr(path_buf)) {
                Some(idx) => *idx,
                None => {
                    // This is assuming that the string in this span represents the entire file
                    // found at `path_buf`.
                    let new_idx = MetadataIndex(context.metadata.insert(Metadatum::FileLocation(
                        path_buf.clone(),
                        span.src().clone(),
                    )));
                    context
                        .metadata_reverse_map
                        .insert(Arc::as_ptr(path_buf), new_idx);
                    new_idx
                }
            };

            MetadataIndex(context.metadata.insert(Metadatum::Span {
                loc_idx,
                start: span.start(),
                end: span.end(),
            }))
        })
    }

    pub fn to_span(&self, context: &Context) -> Result<Span, IrError> {
        match &context.metadata[self.0] {
            Metadatum::Span {
                loc_idx,
                start,
                end,
            } => {
                let (path, src) = match &context.metadata[loc_idx.0] {
                    Metadatum::FileLocation(path, src) => Ok((path.clone(), src.clone())),
                    _otherwise => Err(IrError::InvalidMetadatum),
                }?;
                Span::new(src, *start, *end, Some(path)).ok_or(IrError::InvalidMetadatum)
            }
            _otherwise => Err(IrError::InvalidMetadatum),
        }
    }

    pub fn from_state_idx(context: &mut Context, state_idx: usize) -> Option<MetadataIndex> {
        Some(MetadataIndex(
            context.metadata.insert(Metadatum::StateIndex(state_idx)),
        ))
    }

    pub fn to_state_idx(&self, context: &Context) -> Result<usize, IrError> {
        match &context.metadata[self.0] {
            Metadatum::StateIndex(ix) => Ok(*ix),
            _otherwise => Err(IrError::InvalidMetadatum),
        }
    }

    pub fn get_storage_index(context: &mut Context, storage_op: StorageOperation) -> MetadataIndex {
        *context
            .metadata_storage_indices
            .entry(storage_op)
            .or_insert_with(|| {
                MetadataIndex(
                    context
                        .metadata
                        .insert(Metadatum::StorageAttribute(storage_op)),
                )
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StorageOperation {
    Reads,
    Writes,
    ReadsWrites,
}

use std::fmt::{Display, Error, Formatter};

impl Display for StorageOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self.simple_string())
    }
}

impl StorageOperation {
    pub fn simple_string(&self) -> &'static str {
        match self {
            StorageOperation::Reads => "read",
            StorageOperation::Writes => "write",
            StorageOperation::ReadsWrites => "readwrite",
        }
    }
}