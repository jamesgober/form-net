//! Reading and building query strings.
//!
//! Run with `cargo run --example query_string`.

use form_net::FormError;

fn main() -> Result<(), FormError> {
    let query = form_net::parse_query(b"?q=rust+forms&page=2&tag=a&tag=b")?;
    println!("q:    {:?}", query.get("q"));
    println!("page: {:?}", query.get("page"));
    println!("tags: {:?}", query.get_all("tag").collect::<Vec<_>>());

    // Building one for a redirect or a link.
    let mut url = String::from("/search?");
    form_net::Serializer::new(&mut url)
        .append_pair("q", "rust & forms")
        .append_pair("page", "3");
    println!("next: {url}");
    Ok(())
}
