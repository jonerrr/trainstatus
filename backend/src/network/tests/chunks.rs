use super::{CBOR_VEC_OVERHEAD, CHUNK_PAYLOAD_MAX, chunk_ranges_for_codec};

#[test]
fn shard_ranges_cover_records_in_order_without_exceeding_the_encoded_budget() {
    let lengths = [
        CHUNK_PAYLOAD_MAX - CBOR_VEC_OVERHEAD,
        1,
        CHUNK_PAYLOAD_MAX / 2,
        CHUNK_PAYLOAD_MAX / 2,
        3,
    ];
    let ranges = chunk_ranges_for_codec(&lengths, "records").unwrap();
    let mut next_record = 0;
    for (start, end) in ranges {
        assert_eq!(
            start, next_record,
            "records must not be lost, duplicated or reordered"
        );
        assert!(start < end);
        assert!(lengths[start..end].iter().sum::<usize>() + CBOR_VEC_OVERHEAD <= CHUNK_PAYLOAD_MAX);
        next_record = end;
    }
    assert_eq!(next_record, lengths.len());
}

#[test]
fn a_single_record_must_leave_room_for_its_array_header() {
    assert!(chunk_ranges_for_codec(&[CHUNK_PAYLOAD_MAX], "records").is_err());
}
