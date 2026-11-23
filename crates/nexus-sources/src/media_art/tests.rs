//! Every kind of artwork address, with images made here (never real
//! covers), and the hostile ones: a header claiming gigabytes, a cut-off
//! JPEG, a link, an oversized `data:` address, other schemes.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use super::*;

/// A fresh directory for one test's files.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nexus-art-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A 60×40 image: the left half red, the right half blue, so the centre
/// crop (40×40) is half and half.
fn pixels() -> image::RgbaImage {
    image::RgbaImage::from_fn(60, 40, |x, _| {
        if x < 30 {
            image::Rgba([230, 20, 20, 255])
        } else {
            image::Rgba([20, 20, 230, 255])
        }
    })
}

fn encoded(format: image::ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    let img = image::DynamicImage::ImageRgba8(pixels());
    let img = if format == image::ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(img.to_rgb8())
    } else {
        img
    };
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.display()).replace(' ', "%20")
}

/// The prepared 33 px square's pixel at (x, y).
fn at(art: &Art, x: usize, y: usize) -> [u8; 4] {
    let (_, sq) = art.squares().next().unwrap();
    sq[(y * 33 + x) * 4..][..4].try_into().unwrap()
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 180 && p[2] < 60
}

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 180 && p[0] < 60
}

/// Red left, blue right, as drawn.
fn looks_right(art: &Art) {
    assert!(is_red(at(art, 4, 16)), "{:?}", at(art, 4, 16));
    assert!(is_blue(at(art, 28, 16)), "{:?}", at(art, 28, 16));
}

fn agent() -> ureq::Agent {
    super::agent()
}

#[test]
fn a_file_address_is_read_and_prepared() {
    let d = dir("file");
    for (name, format) in [
        ("cover art.png", image::ImageFormat::Png),
        ("cover.jpg", image::ImageFormat::Jpeg),
        ("cover.webp", image::ImageFormat::WebP),
    ] {
        let path = d.join(name);
        std::fs::write(&path, encoded(format)).unwrap();
        let url = file_url(&path);
        let art = load(&url, AlbumArt::Local, &agent()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(art.key(), key(&url));
        looks_right(&art);
    }
    // file://localhost/… is this computer too; another host is not.
    let path = d.join("cover.jpg");
    let local = format!("file://localhost{}", path.display());
    assert!(load(&local, AlbumArt::All, &agent()).is_ok());
    let remote = format!("file://nas{}", path.display());
    assert!(load(&remote, AlbumArt::All, &agent()).is_err());
}

#[test]
fn a_data_address_is_decoded() {
    let b64 = base64::engine::general_purpose::STANDARD.encode(encoded(image::ImageFormat::Png));
    let url = format!("data:image/png;base64,{b64}");
    let art = load(&url, AlbumArt::Local, &agent()).unwrap();
    looks_right(&art);
    assert!(
        load("data:image/png,%89PNG", AlbumArt::All, &agent()).is_err(),
        "not base64"
    );
    assert!(load("data:image/png;base64,!!!!", AlbumArt::All, &agent()).is_err());
}

/// Serves `body` once on 127.0.0.1; the request's head comes back.
fn serve(status: u16, body: Vec<u8>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/covers/a.jpg?token=secret",
        listener.local_addr().unwrap()
    );
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream);
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let _ = tx.send(head);
        let mut stream = reader.into_inner();
        let _ = write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(&body);
    });
    (url, rx)
}

#[test]
fn a_web_address_is_fetched_with_nothing_but_a_get() {
    let (url, head) = serve(200, encoded(image::ImageFormat::Jpeg));
    let art = load(&url, AlbumArt::All, &agent()).unwrap();
    looks_right(&art);
    let head = head.recv().unwrap().to_ascii_lowercase();
    assert!(
        head.starts_with("get /covers/a.jpg?token=secret http/1.1"),
        "{head}"
    );
    assert!(
        head.contains(&format!("user-agent: {}", crate::USER_AGENT)),
        "{head}"
    );
    for header in ["cookie", "referer", "authorization", "if-none-match"] {
        assert!(
            !head.contains(&format!("{header}:")),
            "{header} sent: {head}"
        );
    }
    // The log never shows the query.
    assert!(!shown(&url).contains("secret"), "{}", shown(&url));

    let (url, _) = serve(404, Vec::new());
    assert!(
        load(&url, AlbumArt::All, &agent())
            .unwrap_err()
            .contains("404")
    );
}

#[test]
fn local_and_off_fetch_nothing() {
    // Nothing listens: a request would fail differently, and slowly.
    let url = "https://images.example/cover.jpg";
    assert_eq!(wanted(Some(url), AlbumArt::Local), None);
    assert_eq!(wanted(Some(url), AlbumArt::Off), None);
    assert_eq!(wanted(Some("file:///a.png"), AlbumArt::Off), None);
    assert_eq!(wanted(Some(url), AlbumArt::All), Some(url));
    assert!(load(url, AlbumArt::Local, &agent()).is_err());
}

#[test]
fn other_schemes_are_ignored() {
    for url in [
        "ftp://example.org/a.png",
        "smb://nas/a.png",
        "javascript:alert(1)",
        "/home/me/a.png",
        "a.png",
        "",
    ] {
        assert_eq!(wanted(Some(url), AlbumArt::All), None, "{url}");
        assert!(load(url, AlbumArt::All, &agent()).is_err(), "{url}");
    }
    assert_eq!(wanted(None, AlbumArt::All), None);
}

/// A PNG claiming `w`×`h` RGBA of `depth` bits a channel: a signature, a
/// header, and a few bytes of image data, all with good checksums, as far
/// as a decoder reads before it learns the size.
fn png_header(w: u32, h: u32, depth: u8) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in bytes {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        let body = [kind, data].concat();
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        png.extend_from_slice(&body);
        png.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[depth, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    // A zlib stream's start: deflate, then a few stored bytes.
    chunk(
        b"IDAT",
        &[0x78, 0x01, 0x01, 0x04, 0x00, 0xfb, 0xff, 0, 0, 0, 0],
    );
    chunk(b"IEND", &[]);
    png
}

#[test]
fn a_header_claiming_gigabytes_is_refused_before_anything_is_allocated() {
    // 30000 × 25000 RGBA: 3 GB.
    let start = Instant::now();
    let err = decode(&png_header(30_000, 25_000, 8)).unwrap_err();
    assert!(err.contains("limit") || err.contains("dimension"), "{err}");
    // Just inside the side limit but over the memory one: 4096 × 4096 is
    // allowed, a 4096 × 4096 16-bit RGBA image (128 MB) is not.
    let err = decode(&png_header(4096, 4096, 16)).unwrap_err();
    assert!(err.contains("limit") || err.contains("memory"), "{err}");
    assert!(start.elapsed().as_secs() < 2, "refused at once");
}

#[test]
fn a_cut_off_jpeg_is_an_error_or_a_picture_never_a_crash() {
    let jpeg = encoded(image::ImageFormat::Jpeg);
    for keep in [2, 20, 200, jpeg.len() / 2, jpeg.len() - 3] {
        match decode(&jpeg[..keep]) {
            Ok((w, h, rgba)) => {
                assert_eq!((w, h), (60, 40));
                assert_eq!(rgba.len(), 60 * 40 * 4);
            }
            Err(e) => assert!(!e.is_empty()),
        }
    }
    assert!(
        decode(&jpeg[..200]).is_err(),
        "the header alone is not a picture"
    );
    assert!(decode(b"GIF89a....").is_err(), "not a format read here");
}

#[test]
fn a_link_a_directory_or_a_huge_file_is_not_read() {
    let d = dir("hostile");
    let real = d.join("real.png");
    std::fs::write(&real, encoded(image::ImageFormat::Png)).unwrap();
    let link = d.join("link.png");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let err = load(&file_url(&link), AlbumArt::All, &agent()).unwrap_err();
    assert!(err.contains("regular file"), "{err}");
    let err = load(&file_url(&d), AlbumArt::All, &agent()).unwrap_err();
    assert!(err.contains("regular file"), "{err}");
    let big = d.join("big.png");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(MAX_FILE + 1)
        .unwrap();
    let err = load(&file_url(&big), AlbumArt::All, &agent()).unwrap_err();
    assert!(err.contains("larger"), "{err}");
    assert!(load(&file_url(&d.join("gone.png")), AlbumArt::All, &agent()).is_err());
    assert!(load("file:///a%zz.png", AlbumArt::All, &agent()).is_err());
}

#[test]
fn an_oversized_data_address_is_refused_before_decoding() {
    let b64 = "A".repeat(MAX_DATA / 3 * 4 + 1024);
    let url = format!("data:image/png;base64,{b64}");
    let err = load(&url, AlbumArt::All, &agent()).unwrap_err();
    assert!(err.contains("larger"), "{err}");
    // What the log shows of it: the type, not the data.
    assert_eq!(shown(&url), "data:image/png…");
}

#[test]
fn keys_are_short_and_say_nothing_of_the_address() {
    let k = key("https://img.example/cover.jpg?token=secret");
    assert!(k.starts_with("art-") && k.len() == 20, "{k}");
    assert_ne!(k, key("https://img.example/other.jpg"));
}

fn art(name: &str) -> Art {
    Art::prepare(key(name), 40, 40, &nexus_core::art::test_card(40, 40)).unwrap()
}

#[test]
fn the_cache_keeps_about_four_tracks_by_size() {
    let mut cache = Cache::default();
    for i in 0..10 {
        cache.put(&format!("file:///{i}.png"), Arc::new(art(&i.to_string())));
        assert!(cache.bytes() <= CACHE_BYTES || cache.len() == 1);
    }
    assert!((3..=5).contains(&cache.len()), "{}", cache.len());
    assert!(cache.get("file:///9.png").is_some(), "the newest stays");
    assert!(cache.get("file:///0.png").is_none(), "the oldest went");
    // Used again, it is no longer the next to go.
    let oldest = format!("file:///{}.png", 10 - cache.len());
    assert!(cache.get(&oldest).is_some());
    cache.put("file:///new.png", Arc::new(art("new")));
    assert!(cache.get(&oldest).is_some());
}

#[test]
fn artwork_shows_cached_at_once_and_late_answers_only_for_the_track() {
    let (tx, jobs) = mpsc::channel();
    let mut a = Artwork::new(AlbumArt::All, tx);
    let (one, two) = ("file:///one.png", "file:///two.png");

    // Nothing yet: the placeholder, and a load asked for.
    let u = a.track(Some(one));
    assert_eq!((u.art, u.key.as_deref()), (None, Some("")));
    assert_eq!(jobs.try_recv().unwrap().url, one);
    // The same address again asks nothing more.
    assert_eq!(a.track(Some(one)), Update::default());
    assert!(jobs.try_recv().is_err());

    // It arrives: the pixels, then their name.
    let u = a.arrived(one, Some(art(one)));
    assert_eq!(u.key, Some(key(one)));
    assert_eq!(u.art.unwrap().key(), key(one));

    // The next track; its answer comes after the track moved back.
    let u = a.track(Some(two));
    assert_eq!(u.key.as_deref(), Some(""));
    let u = a.track(Some(one));
    assert_eq!(u.key, Some(key(one)), "cached: shown at once");
    assert!(u.art.is_some());
    assert_eq!(
        a.arrived(two, Some(art(two))),
        Update::default(),
        "not the track's"
    );
    // A failure changes nothing: the placeholder stays.
    let u = a.track(Some("file:///three.png"));
    assert_eq!(u.key.as_deref(), Some(""));
    assert_eq!(a.arrived("file:///three.png", None), Update::default());

    // Off: the placeholder, even for what is cached; back on, at once.
    a.set_policy(AlbumArt::Off);
    assert_eq!(a.track(Some(two)).key.as_deref(), None, "already empty");
    a.set_policy(AlbumArt::All);
    let u = a.track(Some(two));
    assert_eq!(u.key, Some(key(two)));
    a.set_policy(AlbumArt::Off);
    assert_eq!(a.track(Some(two)).key.as_deref(), Some(""));
    // No track: nothing.
    a.set_policy(AlbumArt::All);
    a.track(Some(one));
    assert_eq!(a.track(None).key.as_deref(), Some(""));
}

#[test]
fn the_loader_answers_with_the_newest_request() {
    let d = dir("loader");
    let path = d.join("a.png");
    std::fs::write(&path, encoded(image::ImageFormat::Png)).unwrap();
    let (tx, rx) = mpsc::channel();
    let jobs = spawn_loader(move |url, art| tx.send((url, art.is_some())).is_ok());
    let url = file_url(&path);
    jobs.send(Job {
        url: url.clone(),
        policy: AlbumArt::Local,
    })
    .unwrap();
    let (got, ok) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!((got, ok), (url, true));
    jobs.send(Job {
        url: "https://images.example/a.jpg".into(),
        policy: AlbumArt::Local,
    })
    .unwrap();
    let (_, ok) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(!ok, "refused under local");
}
