use novadraw::graphics::{Color, ParseColorError};

#[test]
fn non_ascii_input_returns_the_first_invalid_component_without_panicking() {
    for (input, component) in [
        ("中abc", "red"),
        ("12中a", "green"),
        ("1234é", "blue"),
        ("123456é", "alpha"),
        ("1éabc", "red"),
        ("123éa", "green"),
        ("12345éa", "blue"),
    ] {
        for value in [input.to_owned(), format!("#{input}")] {
            let expected = Err(ParseColorError::InvalidDigit { component });
            assert_eq!(Color::from_hex(&value), expected, "{value:?}");
            assert_eq!(value.parse::<Color>(), expected, "{value:?}");
        }
    }
}

#[test]
fn signed_components_and_whitespace_are_not_hexadecimal_digits() {
    for (input, component) in [
        ("+f0000", "red"),
        ("00+f00", "green"),
        ("0000+f", "blue"),
        ("000000+f", "alpha"),
        (" ff000", "red"),
        ("ff00 0", "blue"),
    ] {
        assert_eq!(
            Color::from_hex(input),
            Err(ParseColorError::InvalidDigit { component }),
            "{input:?}"
        );
    }
}

#[test]
fn all_ascii_component_values_preserve_case_prefix_and_alpha_semantics() {
    for value in u8::MIN..=u8::MAX {
        let expected = Color::rgba8(value, value, value, u8::MAX);
        for input in [
            format!("{value:02x}{value:02x}{value:02x}"),
            format!("#{value:02X}{value:02X}{value:02X}"),
        ] {
            assert_eq!(Color::from_hex(&input), Ok(expected));
        }
        assert_eq!(
            Color::from_hex(&format!("#aBcDeF{value:02X}")),
            Ok(Color::rgba8(0xab, 0xcd, 0xef, value))
        );
    }
    for (input, length) in [("", 0), ("#123", 3), ("é", 2), ("中文abc", 9)] {
        assert_eq!(
            Color::from_hex(input),
            Err(ParseColorError::InvalidLength(length))
        );
    }
}
