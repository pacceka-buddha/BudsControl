//! Phase 1 gate: protocol framing, payload decoders/encoders and the device catalog.
//! Expected byte values are worked out by hand from the layouts in `docs/PROTOCOL.md`.

use buds_core::catalog::{self, MODELS};
use buds_core::protocol::{Frame, FrameParser, cmd, parse_hex, reply, to_hex};
use buds_core::types::*;
use buds_core::{encode, parse};

// ---------------------------------------------------------------- helpers

/// A frame with a LEB128 length, as the buds send for long packets.
fn leb_frame(command: u16, seq: u8, payload: &[u8]) -> Vec<u8> {
    let mut len = payload.len() + 7;
    let mut out = vec![0xAA];
    loop {
        let byte = (len & 0x7F) as u8;
        len >>= 7;
        if len == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
    out.extend_from_slice(&[0, 0, (command & 0xFF) as u8, (command >> 8) as u8, seq]);
    out.extend_from_slice(&[(payload.len() & 0xFF) as u8, (payload.len() >> 8) as u8]);
    out.extend_from_slice(payload);
    out
}

/// A noise-query reply payload reporting `bit`: `00 01 01 <mask lo> <mask hi>`.
fn noise_payload(bit: u8) -> Vec<u8> {
    let mask: u16 = 1 << bit;
    vec![0x00, 0x01, 0x01, (mask & 0xFF) as u8, (mask >> 8) as u8]
}

fn buds4() -> &'static catalog::DeviceModel {
    catalog::lookup("065414").expect("Buds 4 is in the catalog")
}

fn sample_eq() -> CustomEq {
    let hz = [62, 250, 1000, 4000, 8000, 16000];
    let gains = [3, -2, 0, 5, -6, 6];
    CustomEq {
        id: 5,
        name: "My EQ".into(),
        min_gain: -6,
        max_gain: 6,
        bands: hz
            .iter()
            .zip(gains)
            .map(|(&hz, gain)| EqBand { hz, gain })
            .collect(),
    }
}

// ---------------------------------------------------------------- frames

#[test]
fn encodes_battery_request() {
    let frame = Frame::new(cmd::BATTERY, 1, vec![]);
    assert_eq!(frame.encode(), [0xAA, 0x07, 0, 0, 0x06, 0x01, 0x01, 0, 0]);
}

#[test]
fn encodes_set_noise_with_payload() {
    // Transparency is set with bit 2 on the Buds 4: mask 0x04, one byte.
    let frame = Frame::new(cmd::SET_NOISE, 2, encode::set_noise(2));
    assert_eq!(
        frame.encode(),
        [
            0xAA, 0x0A, 0, 0, 0x04, 0x04, 0x02, 0x03, 0x00, 0x01, 0x01, 0x04
        ]
    );
}

#[test]
fn reply_command_sets_high_bit() {
    assert_eq!(reply(cmd::BATTERY), 0x8106);
    assert!(Frame::new(0x8106, 1, vec![]).is_reply_to(cmd::BATTERY));
}

#[test]
fn parses_one_frame() {
    let mut parser = FrameParser::new();
    let frames = parser.feed(&Frame::new(0x8106, 7, vec![0, 1, 2, 3]).encode());
    assert_eq!(frames, vec![Frame::new(0x8106, 7, vec![0, 1, 2, 3])]);
    assert_eq!(parser.pending(), 0);
}

#[test]
fn parses_frame_split_over_three_chunks() {
    let bytes = Frame::new(0x8103, 9, vec![0, 0x14, 0x54, 0x06]).encode();
    let mut parser = FrameParser::new();
    assert!(parser.feed(&bytes[..3]).is_empty());
    assert!(parser.feed(&bytes[3..8]).is_empty());
    assert_eq!(parser.pending(), 8);
    let frames = parser.feed(&bytes[8..]);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].command, 0x8103);
    assert_eq!(frames[0].seq, 9);
    assert_eq!(frames[0].payload, [0, 0x14, 0x54, 0x06]);
    assert_eq!(parser.pending(), 0);
}

#[test]
fn parses_two_frames_in_one_chunk() {
    let mut bytes = Frame::new(0x8106, 1, vec![0, 1, 1, 50]).encode();
    bytes.extend(Frame::new(0x0204, 2, vec![3, 1, 1]).encode());
    let frames = FrameParser::new().feed(&bytes);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].command, 0x8106);
    assert_eq!(frames[1].command, 0x0204);
    assert_eq!(frames[1].payload, [3, 1, 1]);
}

#[test]
fn skips_junk_before_aa() {
    let mut bytes = vec![0x00, 0x13, 0x7F];
    bytes.extend(Frame::new(0x8106, 1, vec![0, 0]).encode());
    let frames = FrameParser::new().feed(&bytes);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].command, 0x8106);
}

#[test]
fn parses_two_byte_leb128_length() {
    let payload: Vec<u8> = (0..200u16).map(|n| n as u8).collect();
    let bytes = leb_frame(0x8112, 4, &payload);
    assert_eq!(&bytes[1..3], [0xCF, 0x01]); // 207 = 0xCF, 0x01
    let frames = FrameParser::new().feed(&bytes);
    assert_eq!(frames, vec![Frame::new(0x8112, 4, payload)]);
}

#[test]
fn keeps_truncated_tail_until_the_rest_arrives() {
    let first = Frame::new(0x8106, 1, vec![0, 0]).encode();
    let second = Frame::new(0x0204, 2, vec![1, 1, 1, 90]).encode();
    let mut chunk = first.clone();
    chunk.extend_from_slice(&second[..5]);
    let mut parser = FrameParser::new();
    let frames = parser.feed(&chunk);
    assert_eq!(frames.len(), 1);
    assert_eq!(parser.pending(), 5);
    let frames = parser.feed(&second[5..]);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].command, 0x0204);
    assert_eq!(parser.pending(), 0);
}

#[test]
fn incomplete_length_varint_waits_for_more_bytes() {
    let bytes = leb_frame(0x8112, 4, &[0u8; 150]);
    let mut parser = FrameParser::new();
    assert!(parser.feed(&bytes[..2]).is_empty()); // AA + first varint byte only
    assert_eq!(parser.pending(), 2);
    assert_eq!(parser.feed(&bytes[2..]).len(), 1);
}

#[test]
fn malformed_length_is_treated_as_noise() {
    // Five continuation bytes (varint too long), then a good frame.
    let mut bytes = vec![0xAA, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    bytes.extend(Frame::new(0x8106, 1, vec![0, 0]).encode());
    let mut parser = FrameParser::new();
    let frames = parser.feed(&bytes);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].command, 0x8106);
    assert_eq!(parser.pending(), 0);

    // A length above the cap is also noise, not something to wait for.
    let mut bytes = vec![0xAA, 0xFF, 0xFF, 0x7F];
    bytes.extend(Frame::new(0x8106, 2, vec![0, 0]).encode());
    let frames = FrameParser::new().feed(&bytes);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].seq, 2);
}

#[test]
fn encode_then_parse_round_trips_short_payloads() {
    for len in [0usize, 1, 5, 60, 120] {
        let payload: Vec<u8> = (0..len).map(|n| (n * 7) as u8).collect();
        let frame = Frame::new(0x8109, 200, payload);
        let parsed = FrameParser::new().feed(&frame.encode());
        assert_eq!(parsed, vec![frame], "payload length {len}");
    }
}

#[test]
fn parser_survives_arbitrary_bytes() {
    // Deterministic pseudo-random noise in random-sized chunks: must never panic or hang, and the
    // parser must always be able to consume a good frame afterwards.
    let mut state: u32 = 0x1234_5678;
    let mut next = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 16) as usize
    };
    let mut parser = FrameParser::new();
    for _ in 0..2000 {
        let n = 1 + next() % 40;
        let chunk: Vec<u8> = (0..n)
            .map(|_| if next() % 6 == 0 { 0xAA } else { next() as u8 })
            .collect();
        let _ = parser.feed(&chunk);
    }
    // Flush whatever incomplete frame the noise left behind, then check a good frame parses.
    let _ = parser.feed(&vec![0u8; 70_000]);
    let frames = parser.feed(&Frame::new(0x8106, 1, vec![0, 0]).encode());
    assert!(frames.iter().any(|f| f.command == 0x8106 && f.seq == 1));
}

// ---------------------------------------------------------------- decoders

#[test]
fn decodes_product_id() {
    assert_eq!(
        parse::product_id(&[0x00, 0x14, 0x54, 0x06]).as_deref(),
        Some("065414")
    );
    assert_eq!(parse::product_id(&[0x01, 0x14, 0x54, 0x06]), None); // non-zero status
    assert_eq!(parse::product_id(&[0x00, 0x14, 0x54]), None); // too short
}

#[test]
fn decodes_battery_with_charging_bit_and_empty_case() {
    // left 80% idle, right 55% charging, case empty and idle
    let update = parse::battery(&[0x00, 0x03, 1, 80, 2, 0x80 | 55, 3, 0]).unwrap();
    assert_eq!(
        update.left,
        Some(Battery {
            level: 80,
            charging: false
        })
    );
    assert_eq!(
        update.right,
        Some(Battery {
            level: 55,
            charging: true
        })
    );
    assert_eq!(update.case, Some(None)); // reported but empty: clears the display

    // case at 40% charging; left only mentioned in a later payload
    let update = parse::battery(&[0x00, 0x01, 3, 0x80 | 40]).unwrap();
    assert_eq!(update.left, None);
    assert_eq!(
        update.case,
        Some(Some(Battery {
            level: 40,
            charging: true
        }))
    );

    // 0% but charging still counts as a real case reading
    let update = parse::battery(&[0x00, 0x01, 3, 0x80]).unwrap();
    assert_eq!(
        update.case,
        Some(Some(Battery {
            level: 0,
            charging: true
        }))
    );
}

#[test]
fn battery_push_uses_the_same_layout() {
    // 0x0204 push: type byte 1 in the status slot.
    let update = parse::battery(&[0x01, 0x02, 1, 70, 2, 71]).unwrap();
    assert_eq!(update.left.unwrap().level, 70);
    assert_eq!(update.right.unwrap().level, 71);
}

#[test]
fn truncated_battery_payload_stops_early() {
    let update = parse::battery(&[0x00, 0x03, 1, 80, 2]).unwrap();
    assert_eq!(update.left.unwrap().level, 80);
    assert_eq!(update.right, None);
    assert_eq!(parse::battery(&[0x00]), None);
}

#[test]
fn decodes_capability_bits() {
    // byte 0 = 0b0010_1000 -> bits 3 and 5; byte 4 = 0x04 -> bit 34
    let caps = parse::capabilities(&[0x00, 0x28, 0, 0, 0, 0x04]).unwrap();
    assert!(caps.contains(&3) && caps.contains(&5) && caps.contains(&34));
    assert_eq!(caps.len(), 3);
    assert_eq!(parse::capabilities(&[0x01, 0xFF]), None); // error status
    assert_eq!(parse::capabilities(&[0x00]), None); // no bitmap
    assert!(DeviceFeature::ALL.iter().all(|f| {
        let bit = f.capability_bit();
        [3, 5, 7, 8, 10, 34].contains(&bit)
    }));
}

#[test]
fn buds4_reports_modes_through_its_reported_layout() {
    let m = buds4();
    let known = |bit: u8| parse::noise_report(m, &noise_payload(bit));
    // Transparency is *set* with bit 2 but reported as bit 8.
    assert!(matches!(
        known(8),
        parse::NoiseReport::Known {
            mode: NoiseMode::Transparency,
            level: None,
            ..
        }
    ));
    // Off is reported as bit 3 (set with bit 0).
    assert!(matches!(
        known(3),
        parse::NoiseReport::Known {
            mode: NoiseMode::Off,
            ..
        }
    ));
    // Bit 4 is ANC Max.
    assert!(matches!(
        known(4),
        parse::NoiseReport::Known {
            mode: NoiseMode::Anc,
            level: Some(AncLevel::Max),
            mode_type: 4,
            ..
        }
    ));
    // Bit 11 is ANC Adaptive.
    assert!(matches!(
        known(11),
        parse::NoiseReport::Known {
            mode: NoiseMode::Anc,
            level: Some(AncLevel::Adaptive),
            ..
        }
    ));
    // Bit 1 is plain ANC (type 5): a mode, but no named level.
    assert!(matches!(
        known(1),
        parse::NoiseReport::Known {
            mode: NoiseMode::Anc,
            level: None,
            mode_type: 5,
            ..
        }
    ));
}

#[test]
fn buds_pro3_reports_levels_in_high_bits() {
    let pro3 = catalog::lookup("064014").expect("Buds Pro 3 is in the catalog");
    let report = parse::noise_report(pro3, &noise_payload(13));
    assert!(matches!(
        report,
        parse::NoiseReport::Known {
            mode: NoiseMode::Anc,
            level: Some(AncLevel::Moderate),
            bit: 13,
            ..
        }
    ));
}

#[test]
fn unknown_or_empty_noise_masks_are_ignored() {
    let m = buds4();
    assert_eq!(
        parse::noise_report(m, &noise_payload(9)),
        parse::NoiseReport::UnknownBit(9)
    );
    assert_eq!(
        parse::noise_report(m, &[0x00, 0x01, 0x01, 0x00, 0x00]),
        parse::NoiseReport::None
    );
    assert_eq!(
        parse::noise_report(m, &[0x00, 0x01]),
        parse::NoiseReport::None
    );
    // The lowest set bit wins when several are set.
    assert_eq!(
        parse::noise_mask(&[0x00, 0x01, 0x01, 0b0001_0100]),
        Some(0b0001_0100)
    );
    let report = parse::noise_report(m, &[0x00, 0x01, 0x01, 0b0001_0100]);
    assert!(matches!(report, parse::NoiseReport::Known { bit: 2, .. }));
}

#[test]
fn set_noise_payload_masks() {
    assert_eq!(encode::set_noise(0), [0x01, 0x01, 0x01]);
    assert_eq!(encode::set_noise(2), [0x01, 0x01, 0x04]);
    assert_eq!(encode::set_noise(7), [0x01, 0x01, 0x80]);
    assert_eq!(encode::set_noise(8), [0x01, 0x01, 0x00, 0x01]);
    assert_eq!(encode::set_noise(11), [0x01, 0x01, 0x00, 0x08]);
}

#[test]
fn small_payload_builders() {
    assert_eq!(encode::SUBSCRIBE, [0x03, 0x01, 0x02, 0x03]);
    assert_eq!(encode::NOISE_QUERY, [0x01, 0x01]);
    assert_eq!(encode::EQ_LIST_QUERY, [0x01, 0x05]);
    assert_eq!(encode::LOW_LATENCY_QUERY, [0x01, 0x06]);
    assert_eq!(encode::set_switch(6, true), [6, 1]);
    assert_eq!(encode::set_switch(6, false), [6, 0]);
    assert_eq!(encode::set_eq(3), [3]);
    assert_eq!(encode::find_buds(true), [1]);
    assert_eq!(encode::find_buds(false), [0]);
    assert_eq!(
        encode::set_gesture(EarSide::Right, Gesture::Hold, GestureAction::Next),
        [0x01, 0x02, 0x01, 0x04, 0x06]
    );
}

#[test]
fn custom_eq_round_trips_with_negative_gains() {
    let eq = sample_eq();
    let payload = encode::set_custom_eq(&eq);
    assert_eq!(&payload[..5], [0x02, 0xFA, 0x06, 5, 5]); // action, min -6, max 6, id, name len
    assert_eq!(&payload[5..10], b"My EQ");
    assert_eq!(payload[10], 6); // band count
    assert_eq!(&payload[11..14], [62, 0, 3]); // 62 Hz, +3 dB
    assert_eq!(&payload[14..17], [250, 0, 0xFE]); // 250 Hz, -2 dB

    // A list reply has the same entry layout with `selected` where the set command has `02`.
    let mut reply = vec![0x00, 0x01];
    reply.extend(&payload);
    let decoded = parse::eq_list(&reply).unwrap();
    assert_eq!(decoded, vec![eq]);
}

#[test]
fn eq_list_names_and_bands() {
    let mut eq = sample_eq();
    eq.name = String::new();
    eq.bands[3].hz = 12_000; // needs the high byte
    let mut reply = vec![0x00, 0x01];
    reply.extend(encode::set_custom_eq(&eq));
    let decoded = parse::eq_list(&reply).unwrap();
    assert_eq!(decoded[0].name, "Custom"); // empty name is shown as Custom
    assert_eq!(decoded[0].bands[3].hz, 12_000);
}

#[test]
fn truncated_eq_list_never_panics() {
    let mut reply = vec![0x00, 0x02];
    reply.extend(encode::set_custom_eq(&sample_eq()));
    reply.extend(encode::set_custom_eq(&sample_eq()));
    for end in 0..=reply.len() {
        let _ = parse::eq_list(&reply[..end]);
    }
    assert_eq!(parse::eq_list(&reply).unwrap().len(), 2);
}

#[test]
fn apply_gains_clamps_to_the_range() {
    let mut eq = sample_eq();
    encode::apply_gains(&mut eq, &[10, -10, 1]);
    assert_eq!(eq.bands[0].gain, 6);
    assert_eq!(eq.bands[1].gain, -6);
    assert_eq!(eq.bands[2].gain, 1);
    assert_eq!(eq.bands[3].gain, 5); // untouched: fewer gains than bands
}

#[test]
fn gestures_keep_only_music_context() {
    let p = [
        0x00, 0x04, //
        1, 0x01, 2, 0x06, // left double tap -> next
        1, 0x06, 2, 0x06, // call context: ignored
        2, 0x01, 4, 0x05, // right hold -> previous
        2, 0x01, 3, 0x7F, // unknown action: ignored
    ];
    let g = parse::gestures(&p).unwrap();
    assert_eq!(
        g,
        vec![
            (EarSide::Left, Gesture::DoubleTap, GestureAction::Next),
            (EarSide::Right, Gesture::Hold, GestureAction::Previous),
        ]
    );
    assert_eq!(parse::gestures(&[0x01, 0x00]), None); // error status
}

#[test]
fn hosts_have_reversed_addresses() {
    let mut p = vec![0x00, 0x02];
    // 11:22:33:44:55:66, reserved, connected, this host, "Phone"
    p.extend([0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00, 0x02, 0x00, 5]);
    p.extend(b"Phone");
    // AA:BB:CC:DD:EE:FF, reserved, not connected, this host, "PC"
    p.extend([0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA, 0x00, 0x01, 0x01, 2]);
    p.extend(b"PC");
    let hosts = parse::hosts(&p).unwrap();
    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0].address, "11:22:33:44:55:66");
    assert_eq!(hosts[0].name, "Phone");
    assert!(hosts[0].connected && !hosts[0].is_this_host);
    assert_eq!(hosts[1].address, "AA:BB:CC:DD:EE:FF");
    assert!(!hosts[1].connected && hosts[1].is_this_host);
}

#[test]
fn wear_and_switches_and_eq_selection() {
    let w = parse::wear(&[0x00, 0x03, 1, 3, 2, 1, 1, 2]).unwrap();
    assert_eq!(w[0], (1, Some(Wear::InEar)));
    assert_eq!(w[1], (2, Some(Wear::OutOfEar)));
    assert_eq!(w[2], (1, None)); // 2 is not a known wear value

    assert_eq!(parse::switches(&[0x00, 0x01, 6, 1]), Some(vec![(6, true)]));
    assert_eq!(parse::switches(&[0x00, 0x01, 6, 0]), Some(vec![(6, false)]));
    assert_eq!(parse::switches(&[0x01, 0x01, 6, 1]), None);

    assert_eq!(parse::eq_selected(&[0x00, 0x02]), Some(2));
    assert_eq!(parse::eq_selected(&[0x01, 0x02]), None);
}

#[test]
fn decoders_never_panic_on_any_prefix() {
    let samples: Vec<Vec<u8>> = vec![
        vec![0x00, 0x03, 1, 80, 2, 0xB7, 3, 0],
        vec![0x00, 0x28, 0, 0, 0, 0x04],
        noise_payload(11),
        vec![0x00, 0x02, 1, 1, 2, 6, 2, 1, 4, 5],
        vec![
            0x00, 0x01, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0, 2, 1, 3, b'a', b'b', b'c',
        ],
        vec![0x00, 0x01, 0, 0xFA, 6, 5, 255, b'x'],
    ];
    let m = buds4();
    for s in &samples {
        for end in 0..=s.len() {
            let p = &s[..end];
            let _ = parse::product_id(p);
            let _ = parse::capabilities(p);
            let _ = parse::battery(p);
            let _ = parse::wear(p);
            let _ = parse::noise_report(m, p);
            let _ = parse::switches(p);
            let _ = parse::eq_selected(p);
            let _ = parse::eq_list(p);
            let _ = parse::gestures(p);
            let _ = parse::hosts(p);
        }
    }
}

// ---------------------------------------------------------------- catalog

#[test]
fn catalog_has_105_sorted_unique_models() {
    assert_eq!(MODELS.len(), 105);
    assert!(
        MODELS.windows(2).all(|w| w[0].0 < w[1].0),
        "sorted and unique"
    );
    assert!(MODELS.iter().all(|(pid, _)| pid.len() == 6));
}

#[test]
fn buds4_catalog_entry() {
    let m = buds4();
    assert_eq!(m.name, "OnePlus Buds 4");
    assert_eq!(m.brand, Brand::Oneplus);
    assert_eq!(m.mode_types, [1, 2, 3, 4, 7, 8, 10]);
    assert_eq!(m.eq_presets, [(0, 11), (1, 14), (2, 12)]);
    assert!(m.allows(DeviceFeature::LowLatency) && m.allows(DeviceFeature::Gestures));
    // bit_for_type picks the lowest proven set bit.
    assert_eq!(m.bit_for_type(mode_type::OFF), Some(0));
    assert_eq!(m.bit_for_type(mode_type::TRANSPARENCY), Some(2));
    assert_eq!(m.bit_for_type(AncLevel::Max.mode_type()), Some(4));
    assert_eq!(m.bit_for_type(AncLevel::Adaptive.mode_type()), Some(11));
    assert_eq!(m.bit_for_type(99), None);
}

#[test]
fn catalog_features_list_semantics() {
    // Models the source doesn't describe have `features: None` and allow everything.
    let unknown_features = MODELS.iter().filter(|(_, m)| m.features.is_none()).count();
    assert_eq!(unknown_features, 23);
    assert!(catalog::GENERIC.allows(DeviceFeature::Equalizer));
    // A model with an empty list allows nothing.
    let none = catalog::lookup("050410").unwrap();
    assert!(!none.allows(DeviceFeature::NoiseControl));
}

#[test]
fn unknown_models_fall_back_to_generic_layouts() {
    // Unknown OnePlus/OPPO id: generic Off / Transparency / ANC.
    let m = catalog::model(Some("999910"), Some("OnePlus Buds"));
    assert_eq!(m.mode_types, [1, 2, 5]);
    assert_eq!(m.brand, Brand::Oppo);
    // Unknown id ending in 12 (or 52) is realme.
    assert_eq!(catalog::model(Some("999912"), None).brand, Brand::Realme);
    assert_eq!(catalog::model(Some("999952"), None).brand, Brand::Realme);
    // ... and so is anything named realme.
    assert_eq!(
        catalog::model(None, Some("realme Buds Air 5")).brand,
        Brand::Realme
    );
    // No information at all: generic.
    assert_eq!(catalog::model(None, None).mode_types, [1, 2, 5]);
    // Known ids win over the heuristics.
    assert_eq!(
        catalog::model(Some("065414"), Some("realme")).name,
        "OnePlus Buds 4"
    );
}

// ---------------------------------------------------------------- types and helpers

#[test]
fn level_and_mode_type_mappings() {
    for level in AncLevel::ALL {
        assert_eq!(AncLevel::from_mode_type(level.mode_type()), Some(level));
        assert_eq!(AncLevel::from_key(level.key()), Some(level));
    }
    assert_eq!(AncLevel::from_mode_type(5), None);
    assert_eq!(mode_type::mode(1), NoiseMode::Off);
    assert_eq!(mode_type::mode(2), NoiseMode::Transparency);
    assert_eq!(mode_type::mode(6), NoiseMode::Transparency); // voice enhancement
    assert_eq!(mode_type::mode(5), NoiseMode::Anc);
    assert_eq!(mode_type::name(10), "ANC · Adaptive");
    assert_eq!(mode_type::name(42), "Unknown (type 42)");
}

#[test]
fn wire_codes_round_trip() {
    for a in GestureAction::ALL {
        assert_eq!(GestureAction::from_wire(a.wire()), Some(a));
        assert_eq!(GestureAction::from_key(a.key()), Some(a));
    }
    for g in Gesture::ALL {
        assert_eq!(Gesture::from_wire(g.wire()), Some(g));
        assert_eq!(Gesture::from_key(g.key()), Some(g));
    }
    for s in EarSide::ALL {
        assert_eq!(EarSide::from_wire(s.wire()), Some(s));
    }
    assert_eq!(GestureAction::PlayPause.wire(), 0x01);
    assert_eq!(GestureAction::LowLatency.wire(), 0x11);
    assert_eq!(GestureAction::from_wire(0x7F), None);
    assert_eq!(NoiseMode::from_key("anc"), Some(NoiseMode::Anc));
}

#[test]
fn eq_preset_names() {
    assert_eq!(eq_preset_name(11), "Balanced");
    assert_eq!(eq_preset_name(14), "Vocal");
    assert_eq!(eq_preset_name(12), "Bass");
    assert_eq!(eq_preset_name(99), "Preset 99");
}

#[test]
fn hex_helpers() {
    assert_eq!(to_hex(&[0xAA, 0x07, 0x00]), "AA 07 00");
    assert_eq!(to_hex(&[]), "");
    assert_eq!(parse_hex("06 01"), [0x06, 0x01]);
    assert_eq!(parse_hex("0106"), [0x01, 0x06]);
    assert_eq!(parse_hex("1 2 3"), [0x12]); // odd trailing digit dropped
    assert_eq!(parse_hex("zz"), Vec::<u8>::new());
}
