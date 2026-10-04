//! Synthetic, offline lookup benchmark; does not enumerate or read user files.
use dustaway_core::recognition::recognize;
use std::{hint::black_box, time::Instant};

fn main() {
    let cases = [
        (r"C:\Windows\Installer\cached.msi", false),
        (r"D:\source\node_modules\package\data.bin", false),
        (r"D:\ComfyUI\models\checkpoints\model.safetensors", false),
        (r"D:\SteamLibrary\steamapps\common\Game\movie.mp4", false),
        (r"D:\models\model.gguf", false),
        (r"D:\backup\system.vib", false),
        (r"D:\source\.git\objects", true),
        (r"D:\workspace\ordinary\unknown-file.dat", false),
    ];
    let started = Instant::now();
    black_box(recognize(cases[0].0, cases[0].1));
    println!("catalog initialization: {:?}", started.elapsed());
    let started = Instant::now();
    let mut recognized = 0;
    for _ in 0..10_000 {
        for (path, directory) in &cases {
            recognized += usize::from(black_box(recognize(black_box(path), *directory)).is_some());
        }
    }
    let elapsed = started.elapsed();
    let count = 10_000 * cases.len();
    println!(
        "{count} lookups, {recognized} recognized: {elapsed:?} ({:.2} us/lookup)",
        elapsed.as_secs_f64() * 1_000_000.0 / count as f64
    );
    assert_eq!(recognized, 70_000);
}
