//! Feeding an upload chunk by chunk, as an async server receives it, with a
//! store configured for the route.
//!
//! Run with `cargo run --example upload_session_push`.

use form_net::{FormError, Limits, NoInspector, Policy, Store, UploadSession};

fn main() -> Result<(), FormError> {
    // One store per process (or per route): a private directory, files over
    // 64 KiB on disk, at most 10 MiB on disk per request.
    let store = Store::temp()?
        .memory_threshold(64 * 1024)
        .max_disk_bytes(10 << 20);
    let limits = Limits::DEFAULT.with_max_files(4);
    let content_type = b"multipart/form-data; boundary=X";
    let body = b"--X\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\nsunset\r\n--X\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"a.jpg\"\r\n\r\n\xff\xd8\xff\xe0 jpeg bytes\r\n--X--";

    let mut session = UploadSession::new(content_type, &limits, &Policy::DEFAULT, &store)?;
    // Chunks arrive in any sizes, here 7 bytes at a time.
    for chunk in body.chunks(7) {
        session.push(chunk, &mut NoInspector)?;
    }
    let upload = session.finish(&mut NoInspector)?;

    println!("caption: {:?}", upload.field("caption"));
    for file in upload.files("photo") {
        println!("photo: {} bytes", file.len());
    }
    println!("{:?}", store.stats());
    // Dropping the upload deletes every file it still owns.
    drop(upload);
    println!("after drop: {} files on disk", store.stats().files_on_disk);
    Ok(())
}
