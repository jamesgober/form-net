//! Every driver does the same work: the same total of value bytes for every
//! input, or the comparison would be meaningless.

use bytes::Bytes;
use form_net_bench as b;

#[test]
fn urlencoded_drivers_agree() {
    let mut scratch = vec![0u8; 1 << 20];
    for body in [
        b::login(),
        b::search(),
        b::signup(),
        b::thousand_fields(),
        b::one_mib_plain(),
        b::one_mib_escaped(),
    ] {
        let a = b::fnet_pairs(&body);
        assert_eq!(a, b::fnet_raw(&body, &mut scratch));
        assert_eq!(a, b::fnet_lazy_pairs(&body));
        assert_eq!(a, b::form_urlencoded_sum(&body));
        assert_eq!(a, b::fnet_lazy_sum(&body));
        assert_eq!(a, b::form_urlencoded_pairs(&body));
    }
    assert_eq!(b::fnet_signup(&b::signup()), 36);
    assert_eq!(b::serde_urlencoded_signup(&b::signup()), 36);
    assert_eq!(b::serde_qs_signup(&b::signup()), 36);
    assert_eq!(b::fnet_serialize(), b::form_urlencoded_serialize());
    assert_eq!(b::fnet_serialize(), b::serde_urlencoded_serialize());
}

#[test]
fn multipart_drivers_agree() {
    let ct = b::content_type();
    for body in [
        b::mp_login(),
        b::mp_avatar(),
        b::mp_many_parts(),
        b::mp_all_cr(),
        b::mp_near_miss(),
        b::mp_near_miss_middle(),
        b::mp_batch(),
    ] {
        let shared = Bytes::from(body.clone());
        let a = b::fnet_stream(&body, body.len());
        for chunk in [64 * 1024, 1500, 7] {
            assert_eq!(b::fnet_stream(&body, chunk), a);
            assert_eq!(b::multer_stream(&shared, chunk), a);
        }
        assert_eq!(b::fnet_buffered(&ct, &body), a);
        assert_eq!(b::multipart_018_buffered(&ct, &body), a);
    }
}

#[test]
fn disk_drivers_agree() {
    let ct = b::content_type();
    let body = b::mp_document();
    let store = form_net::Store::temp().unwrap();
    assert_eq!(b::fnet_to_disk(&store, &ct, &body), 2);
    assert_eq!(b::multipart_018_to_disk(&ct, &body), 2);
}
