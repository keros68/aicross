# engine-golden 夹具

这 31 个夹具来自已删除的前端参考实现（`src-ui/src/__tests__/engine-golden.test.ts`，
最后一版见 git 历史）。它们是引擎行为的契约：Rust 侧
`src/engine/golden.rs` 的 `engine_golden_fixtures_replay` 照常重放每一个夹具。

改动夹具须有理由，并同步改 Rust 侧的重放断言。
