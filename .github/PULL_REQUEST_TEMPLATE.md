## Summary

- What changed?
- Why is it needed?

## Safety impact

- Which paths or categories can now be discovered or deleted?
- What lookalikes and protected assets were tested?

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --all-features --locked`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] Positive classifier test added or updated
- [ ] Lookalike/protected-path test added or updated
- [ ] Documentation and changelog updated when behavior changed
- [ ] No secrets, generated build directories, or private scan reports included
