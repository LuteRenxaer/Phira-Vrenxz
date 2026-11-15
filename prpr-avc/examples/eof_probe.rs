use prpr_avc::{AVPixelFormat, Video};
use std::time::{Duration, Instant};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        r"F:\RustProject\phirLie\data\charts\custom\546327cc-99c9-410d-a756-36bf83bd964a\unlock.mp4".to_owned()
    });
    let video = Video::open(&path, AVPixelFormat::YUV420P).unwrap();
    let dur = video.duration();
    println!("duration = {dur}");

    let mut t = 0f64;
    let mut last_pts = -1i64;
    while t < dur + 1.0 {
        let ts = video.elapsed_to_timestamp(t);
        video.seek(ts);
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut got = false;
        loop {
            let changed = video.with_frame(|_, pts| {
                if pts != last_pts {
                    last_pts = pts;
                    true
                } else {
                    false
                }
            });
            if changed {
                got = true;
                break;
            }
            if Instant::now() > deadline {
                println!("TIMEOUT waiting frame at t={t:.3} ts={ts} last_pts={last_pts}");
                std::process::exit(2);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if !got {
            println!("UNEXPECTED no-change");
        }
        if (t * 10.) as i64 % 100 == 0 {
            println!("t={t:.2} pts={last_pts}");
        }
        t += 1.0 / 60.0;
    }
    println!("OK no hang");
}
