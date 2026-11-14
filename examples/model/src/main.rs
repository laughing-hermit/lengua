use std::io::Read as _;

use candle_core::{Device, Tensor};

const LETTERS: usize = 27;
const ROWS: usize = 2 * LETTERS;
const SPACE: usize = 26;
const HIDDEN: usize = 8;
const STEPS: usize = 8;
const GPU: usize = 0;
const FAILED: i32 = 1;
const PROMPT_LIMIT: u64 = 1 << 16;
const ALPHABET: &[u8; LETTERS] = b"abcdefghijklmnopqrstuvwxyz ";

fn device() -> (Device, &'static str) {
    let cuda = candle_core::utils::cuda_is_available()
        .then(|| Device::new_cuda(GPU).ok())
        .flatten();
    if let Some(found) = cuda {
        return (found, "the GPU, with CUDA");
    }
    let metal = candle_core::utils::metal_is_available()
        .then(|| Device::new_metal(GPU).ok())
        .flatten();
    if let Some(found) = metal {
        return (found, "the GPU, with Metal");
    }
    let said = "the CPU: no GPU that this build can use is on this computer";
    debug_assert!(said.starts_with("the CPU"));
    debug_assert!(said.len() < 80);
    (Device::Cpu, said)
}

fn token(letter: char) -> usize {
    let lower = letter.to_ascii_lowercase();
    let index = ALPHABET
        .iter()
        .position(|found| char::from(*found) == lower)
        .unwrap_or(SPACE);
    debug_assert!(index < LETTERS);
    debug_assert!(lower.is_ascii_lowercase() || index == SPACE);
    index
}

fn letter(index: usize) -> char {
    debug_assert!(index < LETTERS);
    let found = ALPHABET.get(index).map_or(' ', |byte| char::from(*byte));
    debug_assert!(found == ' ' || found.is_ascii_lowercase());
    found
}

fn weight(a: usize, b: usize, modulus: usize) -> f32 {
    debug_assert!(modulus > 1);
    let value = u8::try_from((a + b) % modulus).unwrap_or(u8::MAX);
    let centre = u8::try_from(modulus / 2).unwrap_or(u8::MAX);
    debug_assert!(value < 8 && centre < 8);
    f32::from(value) - f32::from(centre)
}

fn weights(device: &Device) -> candle_core::Result<(Tensor, Tensor)> {
    let mut first = Vec::new();
    for row in 0..LETTERS {
        for column in 0..HIDDEN {
            first.push(weight(row * 7, column * 3, 5));
        }
    }
    for row in 0..LETTERS {
        for column in 0..HIDDEN {
            first.push(weight(row * 2, column * 5, 3));
        }
    }
    let mut second = Vec::new();
    for row in 0..HIDDEN {
        for column in 0..LETTERS {
            second.push(weight(row * 5, column * 11, 7));
        }
    }
    debug_assert_eq!(first.len(), ROWS * HIDDEN);
    debug_assert_eq!(second.len(), HIDDEN * LETTERS);
    Ok((
        Tensor::from_vec(first, (ROWS, HIDDEN), device)?,
        Tensor::from_vec(second, (HIDDEN, LETTERS), device)?,
    ))
}

fn highest(scores: &[f32]) -> usize {
    debug_assert!(scores.len() <= LETTERS);
    let mut best = SPACE;
    let mut top = f32::MIN;
    for (index, score) in scores.iter().enumerate() {
        if *score > top {
            top = *score;
            best = index;
        }
    }
    debug_assert!(best < LETTERS);
    best
}

fn next(
    device: &Device,
    model: &(Tensor, Tensor),
    (previous, last): (usize, usize),
) -> candle_core::Result<usize> {
    debug_assert!(previous < LETTERS && last < LETTERS);
    let mut hot = vec![0.0f32; ROWS];
    for place in [last, LETTERS + previous] {
        if let Some(found) = hot.get_mut(place) {
            *found = 1.0;
        }
    }
    let input = Tensor::from_vec(hot, (1, ROWS), device)?;
    let hidden = input.matmul(&model.0)?.relu()?;
    let scores = hidden.matmul(&model.1)?.to_vec2::<f32>()?;
    let best = highest(scores.first().map_or_default(Vec::as_slice));
    debug_assert!(best < LETTERS);
    Ok(best)
}

fn generate(device: &Device, prompt: &str) -> candle_core::Result<String> {
    let model = weights(device)?;
    let mut text = prompt.trim().to_ascii_lowercase();
    let tokens: Vec<usize> = text.chars().map(token).collect();
    let mut last = tokens.last().copied().unwrap_or(SPACE);
    let mut previous = tokens.iter().rev().nth(1).copied().unwrap_or(SPACE);
    for _ in 0..STEPS {
        let found = next(device, &model, (previous, last))?;
        previous = last;
        last = found;
        text.push(letter(found));
    }
    debug_assert!(text.chars().count() >= STEPS);
    debug_assert!(last < LETTERS);
    Ok(text)
}

fn main() {
    let mut prompt = String::new();
    if std::io::stdin()
        .take(PROMPT_LIMIT)
        .read_to_string(&mut prompt)
        .is_err()
    {
        eprintln!("The prompt is not UTF-8 text.");
        std::process::exit(FAILED);
    }
    let (device, said) = device();
    eprintln!("The model runs on {said}.");
    match generate(&device, &prompt) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("The model failed: {error}");
            std::process::exit(FAILED);
        }
    }
}
