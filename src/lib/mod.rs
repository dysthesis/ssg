/// Naïve Fibonacci for demonstration and verification examples.
///
/// This intentionally mirrors the simplest recursive definition so it remains
/// easy to reason about in Lean. It is not stack-safe for large `n` and will
/// overflow `u64` quickly; callers should bound the input accordingly.
///
/// # Examples
///
/// ```
/// assert_eq!(libtemplate::fibonacci(5), 8);
/// ```
#[inline]
#[must_use]
pub fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn base_cases_are_one() {
        assert_eq!(fibonacci(0), 1);
        assert_eq!(fibonacci(1), 1);
    }

    proptest! {
        #[test]
        fn recurrence_holds_for_small_n(n in 0u64..21) {
            prop_assert_eq!(fibonacci(n + 2), fibonacci(n + 1) + fibonacci(n));
        }
    }
}
