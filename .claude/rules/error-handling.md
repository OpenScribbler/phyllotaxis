---
paths:
  - "src/**/*.rs"
---

# Error Handling in Production Code

Never use `.expect()` or `.unwrap()` on fallible operations in production code. Use `.context()` or `.with_context()` from anyhow instead. Panics in a CLI tool are bugs — users should always see a clean error message.

Never use `std::process::exit()`. Return errors via `anyhow::bail!()` or `?` so they propagate to the single handler in `main()`.

These rules do not apply to `#[cfg(test)]` blocks — `.expect()` and `.unwrap()` are fine in tests.
