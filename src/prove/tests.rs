//! Proofs about real programs: the template, examples/add, and planted bugs the samples miss.

use std::time::{Duration, Instant};

use super::*;
use crate::platform::Profile;
use crate::toolchain::Toolchain;

async fn machine(source: &str) -> Machine {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let folder = tempfile::tempdir().expect("temporary folder");
    let (assembly, object, executable) = (
        folder.path().join("main.s"),
        folder.path().join("main.o"),
        folder.path().join("main"),
    );
    std::fs::write(&assembly, source).expect("write");
    toolchain
        .assemble(Profile::LinuxX86_64, &assembly, &object)
        .await
        .expect("assemble");
    toolchain.link(&object, &executable).await.expect("link");
    crate::checker::decode::read(&std::fs::read(&executable).expect("read")).expect("decode")
}

fn block(spec: &str, tests: &str, index: usize) -> Generated {
    let spec = crate::spec::parse(spec).expect("valid spec");
    let label = crate::hash::Sha256Hex::of(b"").label();
    let tests = tests.replacen(
        &tests
            .lines()
            .find(|line| line.starts_with("spec = "))
            .expect("a label")
            .to_owned(),
        &format!("spec = \"{label}\""),
        1,
    );
    let suite = crate::testfile::check(&tests, &spec, &label).expect("valid tests");
    suite.generated[index].clone()
}

fn budget(seconds: u64) -> Budget {
    Budget {
        deadline: Instant::now() + Duration::from_secs(seconds),
        max_paths: 64,
        max_shapes: 2000,
        question_ms: 10_000,
    }
}

fn asset(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/write-assembly/assets")
            .join(name),
    )
    .expect("an asset")
}

#[tokio::test]
async fn proves_the_template_for_every_input() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("z3 isn't installed");
        return;
    }
    let machine = machine(crate::checker::TEMPLATE).await;
    let block = block(&asset("template-spec.md"), &asset("template-tests.toml"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, budget(120));
    eprintln!("{verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

#[test]
fn splits_ranges_into_digit_pieces() {
    let pieces = cover(&digits_of(1000, 4), &digits_of(1234, 4));
    assert_eq!(
        pieces,
        vec![
            vec![(1, 1), (0, 1), (0, 9), (0, 9)],
            vec![(1, 1), (2, 2), (0, 2), (0, 9)],
            vec![(1, 1), (2, 2), (3, 3), (0, 4)],
        ]
    );
    // Every value from -1000 to 1234, and nothing else, in some shape.
    let all = shapes(-1000, 1234);
    let mut count = 0_u64;
    for shape in &all {
        assert!(shape.low >= -1000 && shape.high <= 1234, "{shape:?}");
        count += shape
            .digits
            .iter()
            .map(|(low, high)| u64::from(high - low + 1))
            .product::<u64>();
    }
    assert_eq!(count, 2235);
    assert!(shapes(i128::from(i64::MIN) + 1, i128::from(i64::MAX)).len() < 200);
}

fn example(program: &str, name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(program)
            .join(name),
    )
    .expect("an example file")
}

/// About a minute on 16 cores in release mode: `cargo test --release -- --ignored`.
#[tokio::test]
#[ignore = "slow: proves 4×10^18 pairs of inputs"]
async fn proves_add_for_every_pair_of_inputs() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("add", "main.s")).await;
    let block = block(&example("add", "spec.md"), &example("add", "tests.toml"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, budget(300));
    eprintln!("add: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

#[tokio::test]
async fn finds_a_bug_the_samples_miss() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    // The negative of 1234567 comes out one too big: one input in 18 quintillion.
    let buggy = crate::checker::TEMPLATE
        .replacen(".equ OUT_BUF_LEN, 21\n", ".equ OUT_BUF_LEN, 21\n.equ UNLUCKY, -1234567\n", 1)
        .replacen(
            "    neg rdx\n    jo .Lstart_assert_negate\n",
            "    neg rdx\n    jo .Lstart_assert_negate\n    cmp rdx, UNLUCKY\n    jne .Lstart_lucky\n    \
             inc rdx\n.Lstart_lucky:\n",
            1,
        );
    assert_ne!(buggy, crate::checker::TEMPLATE);
    let machine = machine(&buggy).await;
    let block = block(&asset("template-spec.md"), &asset("template-tests.toml"), 0);
    let verdict = prove(&machine, &block, budget(120));
    assert_eq!(
        verdict,
        Verdict::Counterexample(BTreeMap::from([("n".to_owned(), 1_234_567)]))
    );
}
