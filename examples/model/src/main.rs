use std::io::Read as _;

use candle_core::{Device, Tensor};

const LETTERS: usize = 27;
const HIDDEN: usize = 8;
const STEPS: usize = 8;
const PROMPT_LIMIT: u64 = 1 << 16;

fn device() -> (Device, &'static str) {
    let cuda = candle_core::utils::cuda_is_available()
        .then(|| Device::new_cuda(0).ok())
        .flatten();
    if let Some(found) = cuda {
        return (found, "the GPU, with CUDA");
    }
    let metal = candle_core::utils::metal_is_available()
        .then(|| Device::new_metal(0).ok())
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
    let index = if lower.is_ascii_lowercase() {
        usize::from(u8::try_from(lower).unwrap_or(b'a') - b'a')
    } else {
        LETTERS - 1
    };
    debug_assert!(index < LETTERS);
    debug_assert!(lower.is_ascii_lowercase() || index == LETTERS - 1);
    index
}

fn letter(index: usize) -> char {
    debug_assert!(index < LETTERS);
    let found = match u8::try_from(index) {
        Ok(offset) if index < LETTERS - 1 => char::from(b'a' + offset),
        _ => ' ',
    };
    debug_assert!(found == ' ' || found.is_ascii_lowercase());
    found
}

fn weight(a: usize, b: usize, modulus: usize) -> f32 {
    debug_assert!(modulus > 1);
    let value = u8::try_from((a + b) % modulus).unwrap_or(0);
    let centre = u8::try_from(modulus / 2).unwrap_or(0);
    debug_assert!(value < 8 && centre < 8);
    f32::from(value) - f32::from(centre)
}

fn weights(device: &Device) -> candle_core::Result<(Tensor, Tensor)> {
    let mut first = Vec::with_capacity(2 * LETTERS * HIDDEN);
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
    let mut second = Vec::with_capacity(HIDDEN * LETTERS);
    for row in 0..HIDDEN {
        for column in 0..LETTERS {
            second.push(weight(row * 5, column * 11, 7));
        }
    }
    debug_assert!(first.len() == 2 * LETTERS * HIDDEN);
    debug_assert!(second.len() == HIDDEN * LETTERS);
    Ok((
        Tensor::from_vec(first, (2 * LETTERS, HIDDEN), device)?,
        Tensor::from_vec(second, (HIDDEN, LETTERS), device)?,
    ))
}

fn highest(scores: &[f32]) -> usize {
    debug_assert!(scores.len() <= LETTERS);
    let mut best = 0;
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
    let mut hot = vec![0.0f32; 2 * LETTERS];
    for place in [last, LETTERS + previous] {
        if let Some(found) = hot.get_mut(place) {
            *found = 1.0;
        }
    }
    let input = Tensor::from_vec(hot, (1, 2 * LETTERS), device)?;
    let hidden = input.matmul(&model.0)?.relu()?;
    let scores = hidden.matmul(&model.1)?.to_vec2::<f32>()?;
    let best = highest(scores.first().map(Vec::as_slice).unwrap_or_default());
    debug_assert!(best < LETTERS);
    Ok(best)
}

fn generate(device: &Device, prompt: &str) -> candle_core::Result<String> {
    let model = weights(device)?;
    let mut text = prompt.trim().to_ascii_lowercase();
    let tokens: Vec<usize> = text.chars().map(token).collect();
    let mut last = tokens.last().copied().unwrap_or(LETTERS - 1);
    let mut previous = tokens.iter().rev().nth(1).copied().unwrap_or(LETTERS - 1);
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
        std::process::exit(1);
    }
    let (device, said) = device();
    eprintln!("The model runs on {said}.");
    match generate(&device, &prompt) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("The model failed: {error}");
            std::process::exit(1);
        }
    }
}
