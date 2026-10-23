//! urlencoded cases in the shape of the web-platform-tests urlencoded parser
//! data: each with the WHATWG result and form-net's result, which is the
//! same pairs or a refusal with its reason (strict deviations, FR-5, FR-6).

#![cfg(feature = "alloc")]

use form_net::{EncodingError, FormError, Limits, PercentPolicy, Policy};

enum Expect {
    /// form-net gives the same pairs as WHATWG.
    Same(&'static [(&'static str, &'static str)]),
    /// WHATWG repairs; form-net refuses with this reason, and under
    /// `PercentPolicy::Literal` gives these pairs (or refuses again).
    Refused(
        EncodingError,
        Option<&'static [(&'static str, &'static str)]>,
    ),
}

const CASES: &[(&[u8], Expect)] = &[
    (b"test", Expect::Same(&[("test", "")])),
    (
        "\u{feff}test=\u{feff}".as_bytes(),
        Expect::Same(&[("\u{feff}test", "\u{feff}")]),
    ),
    (
        b"%EF%BB%BFtest=%EF%BB%BF",
        Expect::Same(&[("\u{feff}test", "\u{feff}")]),
    ),
    (
        "\u{2020}&\u{2020}=x".as_bytes(),
        Expect::Same(&[("\u{2020}", ""), ("\u{2020}", "x")]),
    ),
    (b"", Expect::Same(&[])),
    (b"a", Expect::Same(&[("a", "")])),
    (b"a=b", Expect::Same(&[("a", "b")])),
    (b"a=", Expect::Same(&[("a", "")])),
    (b"=b", Expect::Same(&[("", "b")])),
    (b"&", Expect::Same(&[])),
    (b"&a", Expect::Same(&[("a", "")])),
    (b"a&", Expect::Same(&[("a", "")])),
    (b"a&a", Expect::Same(&[("a", ""), ("a", "")])),
    (b"a&b&c", Expect::Same(&[("a", ""), ("b", ""), ("c", "")])),
    (b"a=b&c=d", Expect::Same(&[("a", "b"), ("c", "d")])),
    (b"a=b&c=d&", Expect::Same(&[("a", "b"), ("c", "d")])),
    (b"&&&a=b&&&&c=d&", Expect::Same(&[("a", "b"), ("c", "d")])),
    (
        b"a=a&a=b&a=c",
        Expect::Same(&[("a", "a"), ("a", "b"), ("a", "c")]),
    ),
    (b"a==a", Expect::Same(&[("a", "=a")])),
    (b"a=a+b+c+d", Expect::Same(&[("a", "a b c d")])),
    (
        b"%=a",
        Expect::Refused(EncodingError::InvalidPercent, Some(&[("%", "a")])),
    ),
    (
        b"%a=a",
        Expect::Refused(EncodingError::InvalidPercent, Some(&[("%a", "a")])),
    ),
    (
        b"%a_=a",
        Expect::Refused(EncodingError::InvalidPercent, Some(&[("%a_", "a")])),
    ),
    (b"%61=a", Expect::Same(&[("a", "a")])),
    (b"%61+%4d%4D=", Expect::Same(&[("a MM", "")])),
    (
        b"id=0&value=%",
        Expect::Refused(
            EncodingError::InvalidPercent,
            Some(&[("id", "0"), ("value", "%")]),
        ),
    ),
    (
        b"b=%2sf%2a",
        Expect::Refused(EncodingError::InvalidPercent, Some(&[("b", "%2sf*")])),
    ),
    (
        b"b=%2%2af%2a",
        Expect::Refused(EncodingError::InvalidPercent, Some(&[("b", "%2*f*")])),
    ),
    (
        b"b=%%2a",
        Expect::Refused(EncodingError::InvalidPercent, Some(&[("b", "%*")])),
    ),
    // WHATWG replaces invalid UTF-8 with U+FFFD; form-net refuses under
    // every policy.
    (b"%FE%FF", Expect::Refused(EncodingError::InvalidUtf8, None)),
    (b"%FF%FE", Expect::Refused(EncodingError::InvalidUtf8, None)),
    (b"%C2", Expect::Refused(EncodingError::InvalidUtf8, None)),
    (b"%C2x", Expect::Refused(EncodingError::InvalidUtf8, None)),
    (
        b"_charset_=windows-1252&test=%C2x",
        Expect::Refused(EncodingError::InvalidUtf8, None),
    ),
    (b"\xff=x", Expect::Refused(EncodingError::InvalidUtf8, None)),
    // `;` is not a separator.
    (b"a=1;b=2", Expect::Same(&[("a", "1;b=2")])),
];

fn pairs(input: &[u8], policy: &Policy) -> Result<Vec<(String, String)>, FormError> {
    let form = form_net::parse_urlencoded_with(input, &Limits::DEFAULT, policy)?;
    Ok(form
        .iter()
        .map(|(n, v)| (n.to_owned(), v.to_owned()))
        .collect())
}

fn owned(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn test_whatwg_cases_whole() {
    let literal = Policy::DEFAULT.with_percent(PercentPolicy::Literal);
    for (input, expect) in CASES {
        match expect {
            Expect::Same(p) => {
                assert_eq!(
                    pairs(input, &Policy::DEFAULT).unwrap(),
                    owned(p),
                    "{input:?}"
                );
                assert_eq!(
                    pairs(input, &literal).unwrap(),
                    owned(p),
                    "{input:?} literal"
                );
            }
            Expect::Refused(reason, under_literal) => {
                match pairs(input, &Policy::DEFAULT) {
                    Err(FormError::Encoding { reason: r, .. }) => {
                        assert_eq!(r, *reason, "{input:?}")
                    }
                    other => panic!("{input:?}: expected refusal, got {other:?}"),
                }
                match under_literal {
                    Some(p) => assert_eq!(
                        pairs(input, &literal).unwrap(),
                        owned(p),
                        "{input:?} literal"
                    ),
                    None => assert!(pairs(input, &literal).is_err(), "{input:?} literal"),
                }
            }
        }
    }
}

#[test]
fn test_whatwg_cases_through_every_api_agree() {
    for (input, expect) in CASES {
        let Expect::Same(p) = expect else { continue };
        let lazy: Vec<(String, String)> = form_net::pairs(input)
            .map(|r| r.map(|(n, v)| (n.into_owned(), v.into_owned())))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(lazy, owned(p), "{input:?}");
        let mut raw = Vec::new();
        for pair in form_net::raw_pairs(input, &Limits::DEFAULT) {
            let pair = pair.unwrap();
            let mut n = vec![0u8; pair.name_raw().len()];
            let mut v = vec![0u8; pair.value_raw().len()];
            raw.push((
                pair.decode_name(&mut n, PercentPolicy::Refuse)
                    .unwrap()
                    .to_owned(),
                pair.decode_value(&mut v, PercentPolicy::Refuse)
                    .unwrap()
                    .to_owned(),
            ));
        }
        assert_eq!(raw, owned(p), "{input:?}");
    }
}

#[test]
fn test_serializer_matches_whatwg_and_round_trips() {
    let cases: &[(&str, &str, &str)] = &[
        ("a", "b", "a=b"),
        ("a b", "c d", "a+b=c+d"),
        ("a+b", "c&d", "a%2Bb=c%26d"),
        ("*-._", "~!'()", "*-._=%7E%21%27%28%29"),
        ("", "", "="),
        ("\u{20ac}", "\0", "%E2%82%AC=%00"),
    ];
    for (n, v, expected) in cases {
        let s = form_net::serialize_urlencoded([(n, v)]);
        assert_eq!(s, *expected);
        let back = form_net::parse_urlencoded(s.as_bytes()).unwrap();
        assert_eq!(back.iter().collect::<Vec<_>>(), [(*n, *v)]);
    }
}
