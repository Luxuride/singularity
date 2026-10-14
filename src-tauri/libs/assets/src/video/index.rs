//! Front-loading the seek index of a cached media file.
//!
//! A player can only seek cheaply if it can read the index before it has to
//! read the data. Both common containers write that index last:
//!
//! - mp4 writes `moov` after `mdat` unless the muxer was asked for
//!   `faststart`.
//! - Matroska/WebM writes `Cues` after the last cluster, because the muxer
//!   only learns where the last keyframe is at end-of-stream.
//!
//! GStreamer cannot be leaned on to fix this: `webmmux` has no `faststart`
//! equivalent at all, and the transcode pipelines in this app only reach
//! `mp4mux faststart=true` on the H264 plan. The more important reason to do
//! it here is that cached video is *received*, not produced — `cache_video`
//! stores the sender's bytes verbatim — so the index position is whatever the
//! sender's muxer chose. Rewriting the container on the way in fixes it for
//! every source at the cost of one byte copy and no re-encode.
//!
//! Every entry point returns `None` when the file is already fine or when
//! anything is unexpected, and the caller then keeps the original bytes.
//! Rewriting conservatively is the right failure mode: a video that seeks
//! slowly still plays, whereas a mis-offset one decodes garbage.

/// Relocate a trailing seek index to the front of `bytes`.
///
/// Returns the rewritten file, or `None` when no rewrite is needed or safe.
pub fn front_load_seek_index(bytes: &[u8]) -> Option<Vec<u8>> {
    // mp4 files start with `ftyp`, whose 4-byte type field sits at offset 4.
    if bytes.len() > 8 && bytes[4..8] == *b"ftyp" {
        return front_load_mp4_moov(bytes);
    }

    if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return front_load_matroska_cues(bytes);
    }

    None
}

// ---------------------------------------------------------------------------
// mp4: move `moov` ahead of `mdat`
// ---------------------------------------------------------------------------

/// Boxes whose payload is itself a list of boxes. Only these are descended
/// into, because a blind descent would parse sample data as boxes and rewrite
/// arbitrary bytes as chunk offsets.
const MP4_CONTAINERS: &[&[u8; 4]] = &[
    b"moov", b"trak", b"mdia", b"minf", b"stbl", b"edts", b"dinf", b"mvex", b"moof", b"traf",
    b"udta",
];

#[derive(Debug, Clone, Copy)]
struct Mp4Box {
    kind: [u8; 4],
    start: usize,
    size: usize,
    /// Offset of the box payload, past both the 32- and 64-bit size fields.
    data_start: usize,
}

fn read_mp4_box(bytes: &[u8], pos: usize) -> Option<Mp4Box> {
    let raw_size = u32::from_be_bytes(bytes.get(pos..pos + 4)?.try_into().ok()?);
    let kind: [u8; 4] = bytes.get(pos + 4..pos + 8)?.try_into().ok()?;
    let (size, data_start) = match raw_size {
        1 => (
            u64::from_be_bytes(bytes.get(pos + 8..pos + 16)?.try_into().ok()?) as usize,
            pos + 16,
        ),
        // Size 0 means "extends to end of file".
        0 => (bytes.len() - pos, pos + 8),
        n => (n as usize, pos + 8),
    };
    if size < data_start - pos || pos.checked_add(size)? > bytes.len() {
        return None;
    }
    Some(Mp4Box {
        kind,
        start: pos,
        size,
        data_start,
    })
}

fn mp4_children(bytes: &[u8], range: std::ops::Range<usize>) -> Vec<Mp4Box> {
    let mut out = Vec::new();
    let mut pos = range.start;
    while pos < range.end {
        match read_mp4_box(bytes, pos) {
            Some(box_) if box_.start + box_.size <= range.end => {
                pos = box_.start + box_.size;
                out.push(box_);
            }
            _ => break,
        }
    }
    out
}

/// Add `delta` to every chunk offset in the `stco`/`co64` tables inside
/// `range`. Chunk offsets are absolute file offsets, so moving `mdat` by
/// `delta` moves every chunk by `delta` with it.
fn shift_mp4_chunk_offsets(
    bytes: &mut [u8],
    range: std::ops::Range<usize>,
    shift: &dyn Fn(i64) -> Option<i64>,
) -> Option<()> {
    for box_ in mp4_children(bytes, range) {
        match &box_.kind {
            b"stco" | b"co64" => {
                let entry_size = if &box_.kind == b"stco" { 4 } else { 8 };
                // `stco` payload is version+flags (4) then entry_count (4).
                let table_start = box_.data_start + 8;
                let count = u32::from_be_bytes(
                    bytes
                        .get(box_.data_start + 4..box_.data_start + 8)?
                        .try_into()
                        .ok()?,
                ) as usize;
                let table_end = table_start.checked_add(count.checked_mul(entry_size)?)?;
                if table_end > box_.start + box_.size {
                    return None;
                }
                for at in (table_start..table_end).step_by(entry_size) {
                    let current: i64 = if entry_size == 4 {
                        u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as i64
                    } else {
                        u64::from_be_bytes(bytes[at..at + 8].try_into().ok()?) as i64
                    };
                    let shifted = shift(current)?;
                    if entry_size == 4 {
                        bytes[at..at + 4]
                            .copy_from_slice(&u32::try_from(shifted).ok()?.to_be_bytes());
                    } else {
                        bytes[at..at + 8].copy_from_slice(&(shifted as u64).to_be_bytes());
                    }
                }
            }
            kind if MP4_CONTAINERS.contains(&kind) => {
                shift_mp4_chunk_offsets(bytes, box_.data_start..box_.start + box_.size, &shift)?;
            }
            _ => {}
        }
    }
    Some(())
}

fn front_load_mp4_moov(bytes: &[u8]) -> Option<Vec<u8>> {
    let boxes = mp4_children(bytes, 0..bytes.len());
    let moov = *boxes.iter().find(|b| &b.kind == b"moov")?;
    let mdat = *boxes.iter().find(|b| &b.kind == b"mdat")?;
    if moov.start < mdat.start {
        return None;
    }

    // `ftyp` stays first, because every player expects it there.
    let insert_at = boxes
        .iter()
        .position(|b| &b.kind != b"ftyp")
        .unwrap_or(boxes.len());
    // Everything before the insert point stays put; everything from there to
    // the moov's old position moves later by the moov's size.
    let insert_offset: usize = boxes[..insert_at].iter().map(|b| b.size).sum();

    let mut out = Vec::with_capacity(bytes.len());
    for (index, box_) in boxes.iter().enumerate() {
        if index == insert_at {
            out.extend_from_slice(&bytes[moov.start..moov.start + moov.size]);
        }
        if box_.start == moov.start {
            continue;
        }
        out.extend_from_slice(&bytes[box_.start..box_.start + box_.size]);
    }

    // Chunk offsets are absolute file offsets, so each one shifts by the
    // moov's size only if it pointed into the displaced region. An offset
    // that lands inside the moov itself was never valid.
    let moov_end = moov.start + moov.size;
    shift_mp4_chunk_offsets(
        &mut out,
        insert_offset..insert_offset + moov.size,
        &|offset| {
            if (insert_offset as i64..moov.start as i64).contains(&offset) {
                offset.checked_add(moov.size as i64)
            } else if (moov.start as i64..moov_end as i64).contains(&offset) {
                None
            } else {
                Some(offset)
            }
        },
    )?;

    Some(out)
}

// ---------------------------------------------------------------------------
// Matroska / WebM: move `Cues` ahead of the first cluster
// ---------------------------------------------------------------------------

const SEGMENT_ID: u64 = 0x1853_8067;
const SEEK_HEAD_ID: u64 = 0x114d_9b74;
const CUES_ID: u64 = 0x1c53_bb6b;
const CLUSTER_ID: u64 = 0x1f43_b675;
const SEEK_ID: u64 = 0x4dbb;
const SEEK_POSITION_ID: u64 = 0x53ac;
// Element IDs are stored with their length-marker bit kept, so the registered
// values decode as themselves: the 1-byte CuePoint ID is 0xBB, the 1-byte
// CueTrackPositions ID is 0xB7, and the 1-byte CueClusterPosition ID is 0xF1.
const CUE_POINT_ID: u64 = 0xbb;
const CUE_TRACK_POSITIONS_ID: u64 = 0xb7;
const CUE_CLUSTER_POSITION_ID: u64 = 0xf1;

/// The width the rewritten Segment size field is written at. Fixing it means a
/// changed segment length can never change a vint width, so nothing shifts
/// as a side effect of the size change.
const SEGMENT_SIZE_WIDTH: usize = 8;

/// Read an EBML variable-size integer. `keep_marker` keeps the length marker,
/// which is part of an element ID but not part of a size or a value.
fn read_vint(bytes: &[u8], pos: usize, keep_marker: bool) -> Option<(u64, usize)> {
    let first = *bytes.get(pos)?;
    if first == 0 {
        return None;
    }
    let mut width = 1usize;
    let mut mask = 0x80u8;
    while first & mask == 0 {
        mask >>= 1;
        width += 1;
    }
    let raw = bytes.get(pos..pos + width)?;
    let mut value = if keep_marker {
        u64::from(first)
    } else {
        u64::from(first & (mask - 1))
    };
    for byte in &raw[1..] {
        value = (value << 8) | u64::from(*byte);
    }
    Some((value, width))
}

/// Write `value` as a `width`-byte EBML variable-size integer. Returns `None`
/// when the value does not fit, which is the signal to abandon the rewrite
/// rather than widen an element and cascade size changes up the tree.
fn write_vint(value: u64, width: usize, keep_marker: bool) -> Option<Vec<u8>> {
    if !(1..=8).contains(&width) {
        return None;
    }
    // The marker bit for a `width`-byte vint sits at bit 7*width: 0x80 for
    // one byte, 0x4000 for two, and so on.
    let marker = 1u64 << (7 * width);
    if keep_marker {
        // An ID carries its marker bit, so it must sit in [marker, 2*marker).
        if value < marker || value >= 2 * marker {
            return None;
        }
    } else if value >= marker {
        return None;
    }
    let raw = if keep_marker { value } else { value | marker };
    Some(raw.to_be_bytes()[8 - width..].to_vec())
}

/// An EBML element. `id` is the decoded value with its length marker, which is
/// how EBML defines an ID; the class-2 IDs involved here are 2 bytes wide, so
/// they are compared as values rather than as byte arrays.
#[derive(Debug, Clone)]
struct EbmlElement {
    id: u64,
    id_width: usize,
    size_width: usize,
    start: usize,
    data_start: usize,
    end: usize,
}

impl EbmlElement {
    fn size(&self) -> usize {
        self.end - self.start
    }
}

fn read_ebml_element(bytes: &[u8], pos: usize) -> Option<EbmlElement> {
    let (id_value, id_width) = read_vint(bytes, pos, true)?;
    if !(1..=4).contains(&id_width) {
        return None;
    }
    let (size, size_width) = read_vint(bytes, pos + id_width, false)?;
    let data_start = pos + id_width + size_width;
    // An all-ones size means "unknown length", which runs to the end.
    let unknown = size == (1u64 << (7 * size_width)) - 1;
    let end = if unknown {
        bytes.len()
    } else {
        data_start.checked_add(usize::try_from(size).ok()?)?
    };
    if end > bytes.len() || end < data_start {
        return None;
    }
    Some(EbmlElement {
        id: id_value,
        id_width,
        size_width,
        start: pos,
        data_start,
        end,
    })
}

fn ebml_children(bytes: &[u8], range: std::ops::Range<usize>) -> Vec<EbmlElement> {
    let mut out = Vec::new();
    let mut pos = range.start;
    while pos < range.end {
        match read_ebml_element(bytes, pos) {
            Some(element) if element.end <= range.end => {
                pos = element.end;
                out.push(element);
            }
            _ => break,
        }
    }
    out
}

/// Add `delta` to every `CueClusterPosition` inside `range`. These are stored
/// relative to the start of the segment's data, so uniformly shifting the
/// clusters is a uniform shift of these values.
fn shift_cue_cluster_positions(
    bytes: &mut [u8],
    range: std::ops::Range<usize>,
    delta: i64,
) -> Option<()> {
    for element in ebml_children(bytes, range) {
        if element.id == CUE_CLUSTER_POSITION_ID {
            let (value, width) = read_vint(bytes, element.data_start, false)?;
            let shifted = value.checked_add_signed(delta)?;
            let encoded = write_vint(shifted, width, false)?;
            bytes[element.data_start..element.data_start + width].copy_from_slice(&encoded);
        } else if element.id == CUES_ID
            || element.id == CUE_POINT_ID
            || element.id == CUE_TRACK_POSITIONS_ID
        {
            shift_cue_cluster_positions(bytes, element.data_start..element.end, delta)?;
        }
    }
    Some(())
}

/// Add `delta` to every `SeekPosition` in the `SeekHead` inside `range`,
/// keeping each field's existing width.
fn shift_seek_positions(bytes: &mut [u8], range: std::ops::Range<usize>, delta: i64) -> Option<()> {
    for element in ebml_children(bytes, range) {
        if element.id != SEEK_ID {
            continue;
        }
        for field in ebml_children(bytes, element.data_start..element.end) {
            if field.id != SEEK_POSITION_ID {
                continue;
            }
            let (value, width) = read_vint(bytes, field.data_start, false)?;
            let shifted = value.checked_add_signed(delta)?;
            let encoded = write_vint(shifted, width, false)?;
            bytes[field.data_start..field.data_start + width].copy_from_slice(&encoded);
        }
    }
    Some(())
}

fn front_load_matroska_cues(bytes: &[u8]) -> Option<Vec<u8>> {
    // Elements outside the Segment keep their bytes and their order.
    let mut prefix: Vec<u8> = Vec::new();
    let mut trailing: Vec<u8> = Vec::new();
    let mut segment: Option<EbmlElement> = None;
    let mut pos = 0usize;
    while pos < bytes.len() {
        let element = read_ebml_element(bytes, pos)?;
        if element.end <= element.start {
            return None;
        }
        if element.id == SEGMENT_ID {
            if segment.is_some() {
                return None;
            }
            segment = Some(element.clone());
        } else if segment.is_some() {
            trailing.extend_from_slice(&bytes[element.start..element.end]);
        } else {
            prefix.extend_from_slice(&bytes[element.start..element.end]);
        }
        pos = element.end;
    }
    let segment = segment?;

    let children = ebml_children(bytes, segment.data_start..segment.end);
    let cues = children.iter().find(|c| c.id == CUES_ID)?.clone();
    let first_cluster = children.iter().find(|c| c.id == CLUSTER_ID)?.clone();
    if cues.start < first_cluster.start {
        return None;
    }

    // Because the Segment size is rewritten at a fixed width, every element
    // inside it moves by one known constant plus, for elements that were
    // behind the first cluster, the size of the hoisted Cues.
    let header_shift = SEGMENT_SIZE_WIDTH as i64 - segment.size_width as i64;
    let cues_size = cues.size();

    let mut out = Vec::with_capacity(bytes.len());
    for child in &children {
        if child.start == cues.start {
            continue;
        }
        if child.start == first_cluster.start {
            out.extend_from_slice(&bytes[cues.start..cues.end]);
        }
        out.extend_from_slice(&bytes[child.start..child.end]);
    }

    // The new start of the segment's data, which every cue position is
    // measured from.
    let new_data_start = segment.start + segment.id_width + SEGMENT_SIZE_WIDTH;
    let cues_element_at = new_data_start
        .checked_add(
            children
                .iter()
                .find(|c| c.start == first_cluster.start)
                .map(|c| c.start - segment.data_start)?,
        )?
        .checked_sub(new_data_start)?;

    shift_cue_cluster_positions(
        &mut out,
        cues_element_at..cues_element_at + cues_size,
        header_shift + cues_size as i64,
    )?;

    // A SeekHead caches the positions of other top-level elements, so each
    // reference shifts by however much its target moved.
    if let Some(seek_head) = children.iter().find(|c| c.id == SEEK_HEAD_ID) {
        let behind = (seek_head.start > first_cluster.start).then_some(cues_size);
        let element_at = (seek_head.start + header_shift as usize + behind.unwrap_or(0))
            .checked_sub(new_data_start)?;
        shift_seek_positions(
            &mut out,
            element_at..element_at + seek_head.size(),
            header_shift + behind.unwrap_or(0) as i64,
        )?;
    }

    let mut result = prefix;
    result.extend_from_slice(&write_vint(segment.id, segment.id_width, true)?);
    result.extend_from_slice(&write_vint(
        (out.len() + trailing.len()) as u64,
        SEGMENT_SIZE_WIDTH,
        false,
    )?);
    result.extend_from_slice(&out);
    result.extend_from_slice(&trailing);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- mp4 fixtures ------------------------------------------------------

    fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    /// A `stco` chunk-offset table, which is what a hoist has to rewrite.
    fn stco(offsets: &[u32]) -> Vec<u8> {
        let mut payload = 1u32.to_be_bytes().to_vec();
        payload.extend_from_slice(&(offsets.len() as u32).to_be_bytes());
        for offset in offsets {
            payload.extend_from_slice(&offset.to_be_bytes());
        }
        mp4_box(b"stco", &payload)
    }

    /// `ftyp` + `mdat` + `moov(stco)`, with the chunk offsets pointing into
    /// `mdat`'s payload.
    fn build_mp4(moov_first: bool) -> Vec<u8> {
        let ftyp = mp4_box(b"ftyp", b"isom\x00\x00\x02\x00");
        let mdat = mp4_box(b"mdat", &[0u8; 8]);
        // moov payload is exactly the stco box, so the table sits at
        // moov.start + 8 + 8.
        let chunk_offsets: [u32; 2] = [0, 0];
        let moov_size = 8 + stco(&chunk_offsets).len();
        // In the pre-hoist layout mdat's payload starts after ftyp and the
        // mdat header; the hoist is expected to shift the offsets by mdat's
        // move to its new home behind ftyp and moov.
        let first_chunk = ftyp.len() + 8;
        let offsets = [first_chunk as u32, (first_chunk + 4) as u32];
        let moov = mp4_box(b"moov", &stco(&offsets));
        debug_assert_eq!(stco(&chunk_offsets).len(), stco(&offsets).len());
        debug_assert_eq!(moov.len(), moov_size);

        let mut out = Vec::new();
        out.extend_from_slice(&ftyp);
        if moov_first {
            out.extend_from_slice(&moov);
            out.extend_from_slice(&mdat);
        } else {
            out.extend_from_slice(&mdat);
            out.extend_from_slice(&moov);
        }
        out
    }

    fn first_chunk_offset(rewritten: &[u8]) -> usize {
        let boxes = mp4_children(rewritten, 0..rewritten.len());
        let moov = boxes.iter().find(|b| b.kind == *b"moov").expect("moov");
        let stco = mp4_children(rewritten, moov.data_start..moov.start + moov.size)
            .into_iter()
            .find(|b| b.kind == *b"stco")
            .expect("stco");
        let at = stco.data_start + 8;
        u32::from_be_bytes(rewritten[at..at + 4].try_into().expect("u32")) as usize
    }

    #[test]
    fn mp4_with_moov_first_is_left_alone() {
        assert_eq!(front_load_seek_index(&build_mp4(true)), None);
    }

    #[test]
    fn mp4_with_trailing_moov_gets_it_hoisted() {
        let rewritten = front_load_seek_index(&build_mp4(false)).expect("rewrite");
        let kinds: Vec<[u8; 4]> = mp4_children(&rewritten, 0..rewritten.len())
            .iter()
            .map(|b| b.kind)
            .collect();
        assert_eq!(kinds, vec![*b"ftyp", *b"moov", *b"mdat"]);
    }

    #[test]
    fn mp4_hoist_keeps_chunk_offsets_addressing_mdat() {
        let rewritten = front_load_seek_index(&build_mp4(false)).expect("rewrite");
        let mdat = mp4_children(&rewritten, 0..rewritten.len())
            .into_iter()
            .find(|b| b.kind == *b"mdat")
            .expect("mdat");
        assert_eq!(first_chunk_offset(&rewritten), mdat.data_start);
    }

    #[test]
    fn mp4_rewrite_preserves_total_size_and_is_idempotent() {
        let original = build_mp4(false);
        let once = front_load_seek_index(&original).expect("rewrite");
        assert_eq!(once.len(), original.len());
        assert_eq!(front_load_seek_index(&once), None);
    }

    // -- Matroska fixtures -------------------------------------------------

    fn elem(id: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&write_vint(payload.len() as u64, 4, false).expect("size"));
        out.extend_from_slice(payload);
        out
    }

    fn uint_elem(id: &[u8], value: u64, width: usize) -> Vec<u8> {
        elem(id, &write_vint(value, width, false).expect("vint"))
    }

    fn wrap(id: &[u8], inner: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&write_vint(inner.len() as u64, 4, false).expect("size"));
        out.extend_from_slice(inner);
        out
    }

    /// Write an element ID from its decoded value at the width EBML gives it.
    fn id_bytes(id: u64) -> Vec<u8> {
        write_vint(id, id_width(id), true).expect("id")
    }

    fn id_width(id: u64) -> usize {
        match id {
            SEEK_ID | SEEK_POSITION_ID => 2,
            // TrackNumber sits alongside the Cue fields at 1 byte.
            CUE_POINT_ID | CUE_TRACK_POSITIONS_ID | CUE_CLUSTER_POSITION_ID | 0xd7 => 1,
            _ => 4,
        }
    }

    /// A WebM whose clusters come first and whose Cues point at them, which is
    /// the layout `webmmux` produces.
    fn build_matroska(cues_first: bool) -> Vec<u8> {
        let header = elem(&id_bytes(0x1a45_dfa3), &[0u8; 16]);
        let cluster = |tag: u8| elem(&id_bytes(CLUSTER_ID), &[tag; 32]);
        let first = cluster(0xaa);
        let second = cluster(0xbb);

        // Cue positions are relative to the start of the segment's data.
        let mut payload = Vec::new();
        for position in [0u64, first.len() as u64] {
            let track = wrap(
                &id_bytes(CUE_TRACK_POSITIONS_ID),
                &[
                    uint_elem(&id_bytes(0xd7), 1, 1),
                    uint_elem(&id_bytes(CUE_CLUSTER_POSITION_ID), position, 4),
                ]
                .concat(),
            );
            payload.extend_from_slice(&wrap(&id_bytes(CUE_POINT_ID), &track));
        }
        let cues = elem(&id_bytes(CUES_ID), &payload);

        let mut body = Vec::new();
        if cues_first {
            body.extend_from_slice(&cues);
        }
        body.extend_from_slice(&first);
        body.extend_from_slice(&second);
        if !cues_first {
            body.extend_from_slice(&cues);
        }

        let mut out = header;
        out.extend_from_slice(&id_bytes(SEGMENT_ID));
        out.extend_from_slice(&write_vint(body.len() as u64, 8, false).expect("size"));
        out.extend_from_slice(&body);
        out
    }

    fn segment_of(bytes: &[u8]) -> EbmlElement {
        read_ebml_element(bytes, 4 + 16 + 4).expect("segment")
    }

    fn cue_positions(bytes: &[u8], segment: &EbmlElement) -> Vec<u64> {
        let cues = ebml_children(bytes, segment.data_start..segment.end)
            .into_iter()
            .find(|c| c.id == CUES_ID)
            .expect("cues");
        let mut positions = Vec::new();
        for cue in ebml_children(bytes, cues.data_start..cues.end) {
            for track in ebml_children(bytes, cue.data_start..cue.end) {
                for field in ebml_children(bytes, track.data_start..track.end) {
                    if field.id == CUE_CLUSTER_POSITION_ID {
                        let (value, _) = read_vint(bytes, field.data_start, false).expect("vint");
                        positions.push(value);
                    }
                }
            }
        }
        positions
    }

    fn cluster_offsets(bytes: &[u8], segment: &EbmlElement) -> Vec<usize> {
        ebml_children(bytes, segment.data_start..segment.end)
            .into_iter()
            .filter(|c| c.id == CLUSTER_ID)
            .map(|c| c.start - segment.data_start)
            .collect()
    }

    #[test]
    fn matroska_with_trailing_cues_gets_them_hoisted() {
        let rewritten = front_load_seek_index(&build_matroska(false)).expect("rewrite");
        let ids: Vec<u64> = ebml_children(
            &rewritten,
            segment_of(&rewritten).data_start..segment_of(&rewritten).end,
        )
        .iter()
        .map(|c| c.id)
        .collect();
        assert_eq!(ids, vec![CUES_ID, CLUSTER_ID, CLUSTER_ID]);
    }

    #[test]
    fn matroska_hoist_preserves_cluster_payloads() {
        let original = build_matroska(false);
        let original_segment = segment_of(&original);
        let payloads: Vec<Vec<u8>> =
            ebml_children(&original, original_segment.data_start..original_segment.end)
                .into_iter()
                .filter(|c| c.id == CLUSTER_ID)
                .map(|c| original[c.data_start..c.end].to_vec())
                .collect();

        let rewritten = front_load_seek_index(&original).expect("rewrite");
        let segment = segment_of(&rewritten);
        let rewritten_payloads: Vec<Vec<u8>> =
            ebml_children(&rewritten, segment.data_start..segment.end)
                .into_iter()
                .filter(|c| c.id == CLUSTER_ID)
                .map(|c| rewritten[c.data_start..c.end].to_vec())
                .collect();
        assert_eq!(rewritten_payloads, payloads);
    }

    #[test]
    fn matroska_cue_positions_still_address_the_clusters() {
        let rewritten = front_load_seek_index(&build_matroska(false)).expect("rewrite");
        let segment = segment_of(&rewritten);
        let positions: Vec<usize> = cue_positions(&rewritten, &segment)
            .into_iter()
            .map(|p| p as usize)
            .collect();
        assert_eq!(positions, cluster_offsets(&rewritten, &segment));
    }

    #[test]
    fn matroska_with_front_cues_is_left_alone() {
        assert_eq!(front_load_seek_index(&build_matroska(true)), None);
    }

    #[test]
    fn vint_round_trips_at_every_width() {
        for width in 1..=8usize {
            let max = (1u64 << (7 * width)) - 1;
            for value in [0u64, 1, max / 2, max] {
                let encoded = write_vint(value, width, false).expect("encode");
                assert_eq!(encoded.len(), width);
                let (decoded, read_width) = read_vint(&encoded, 0, false).expect("decode");
                assert_eq!(decoded, value, "width {width}");
                assert_eq!(read_width, width);
            }
            assert!(write_vint(max + 1, width, false).is_none());
        }
    }

    #[test]
    fn unknown_input_is_ignored() {
        assert_eq!(front_load_seek_index(b"not a media file at all"), None);
        assert_eq!(front_load_seek_index(b"ab"), None);
    }
}
