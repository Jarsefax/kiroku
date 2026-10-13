use kiroku::selection::Selection;

#[test]
fn selections_accept_ranges_lists_and_letters_and_bound_expansion() {
    for (input, expected) in [
        ("1-3", vec![1, 2, 3]),
        ("1,3", vec![1, 3]),
        ("5,1-3,2-4", vec![1, 2, 3, 4, 5]),
        (" 1 - 3 , 5 ", vec![1, 2, 3, 5]),
        ("3-3", vec![3]),
    ] {
        assert_eq!(
            input.parse::<Selection>().unwrap().resolve(5).unwrap(),
            expected
        );
    }
    assert_eq!(
        Selection::letters("Z-AB,A").unwrap().resolve(28).unwrap(),
        [1, 26, 27, 28]
    );
    for invalid in [
        "",
        "0",
        "2-1",
        "1--3",
        "1,",
        ",1",
        "1,,3",
        "+1",
        "1.3",
        "1\n",
        "A",
        "9999999999999999999999999999999999999",
    ] {
        assert!(invalid.parse::<Selection>().is_err(), "{invalid:?}");
    }
    for invalid in ["a", "A-1", "B-A", "A,,C"] {
        assert!(Selection::letters(invalid).is_err(), "{invalid}");
    }
    assert!(
        format!("1-{}", usize::MAX)
            .parse::<Selection>()
            .unwrap()
            .resolve(3)
            .is_err()
    );
}
