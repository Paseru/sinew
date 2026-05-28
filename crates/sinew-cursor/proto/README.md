# agent.v1 protobuf

- `agent.fds` — binary descriptor exported from `scripts/agent-bridge/vendor/agent_pb.ts` (Buf `fileDesc` wire format, not a raw `FileDescriptorSet` for prost).
- Export: `node scripts/export-agent-descriptor.mjs` (after `cd scripts/agent-bridge && npm ci`).
- Next step for the Rust bridge: obtain a `.proto` or standard `FileDescriptorSet` (e.g. `buf export`) then enable `prost-build` in `sinew-cursor`.
