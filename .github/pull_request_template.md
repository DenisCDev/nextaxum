## Summary

<!-- 1-3 bullets: what changed and why -->

## Test plan

- [ ] `cd frontend && npm run lint && npm run typecheck && npm run test:run && npm run build`
- [ ] `cd backend && cargo fmt --check && cargo clippy --locked -- -D warnings && cargo test --locked`
- [ ] (if backend changes) ran migrations against a local Supabase / Postgres
- [ ] (if UI changes) verified at least one user flow in the browser

## Screenshots / logs

<!-- optional but useful for UI or behavioural changes -->

## Notes for reviewers

<!-- migrations, env var changes, things to watch in the deploy preview -->
