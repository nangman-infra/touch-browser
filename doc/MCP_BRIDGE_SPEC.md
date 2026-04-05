# MCP Bridge Spec

- Status: `Experimental`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `stdio MCP bridge over touch-browser serve`

## 1. 목적

이 문서는 `touch-browser serve` 위에 얇은 MCP bridge를 올려 외부 agent가 MCP tool server처럼 붙을 수 있는 표면을 고정합니다.

현재 제공 파일:

- [touch-browser-mcp-bridge.mjs](../scripts/touch-browser-mcp-bridge.mjs)

실행:

- `pnpm run mcp:bridge`

빠른 설정 예시:

```json
{
  "mcpServers": {
    "touch-browser": {
      "command": "node",
      "args": ["scripts/touch-browser-mcp-bridge.mjs"]
    }
  }
}
```

## 2. 현재 프로토콜 범위

bridge는 MCP stdio JSON-RPC의 최소 subset만 구현합니다.

지원:

- `initialize`
- `ping`
- `tools/list`
- `tools/call`

현재 tool 목록:

- `tb_status`
- `tb_session_create`
- `tb_open`
- `tb_extract`
- `tb_policy`
- `tb_tab_open`
- `tb_tab_list`
- `tb_tab_select`
- `tb_tab_close`
- `tb_click`
- `tb_type`
- `tb_type_secret`
- `tb_submit`
- `tb_refresh`
- `tb_checkpoint`
- `tb_profile`
- `tb_profile_set`
- `tb_approve`
- `tb_secret_store`
- `tb_secret_clear`
- `tb_telemetry_summary`
- `tb_telemetry_recent`
- `tb_session_synthesize`
- `tb_session_close`

## 3. 현재 검증

- [mcp-bridge-smoke.test.ts](../evals/src/runtime/mcp-bridge-smoke.test.ts)
- [interface-compatibility.test.ts](../evals/src/runtime/interface-compatibility.test.ts)
- [serve-daemon.test.ts](../evals/src/runtime/serve-daemon.test.ts)
- `initialize -> tools/list -> tools/call(tb_status)` round-trip 검증 완료
- `runtime.session.click` / `runtime.session.type` / `runtime.session.submit` daemon 경로 검증 완료
- `runtime.session.typeSecret` / `runtime.session.secret.store` / `runtime.session.refresh` daemon 경로 검증 완료
- `runtime.session.checkpoint` / `runtime.session.approve` daemon 경로 검증 완료
- `runtime.session.profile.get` / `runtime.session.profile.set` daemon 경로 검증 완료
- `runtime.telemetry.summary` / `runtime.telemetry.recent` daemon 경로 검증 완료
- fixture-backed reference workflow artifact 생성 경로 제공
- staged public/trusted-source workflow artifact 생성 경로 제공
- public-web reference workflow artifact 생성 경로 제공

## 4. 현재 한계

- full MCP schema negotiation이나 resource/prompt surface는 아직 없습니다.
- tool set이 serve 전체 메서드를 모두 노출하지는 않습니다.
- interactive tool은 현재 allowlisted daemon session 안에서만 의미가 있으며, challenge/mfa/auth/high-risk-write signal에는 별도 ack가 필요합니다.
- `tb_checkpoint`는 provider hint와 required ack risk뿐 아니라 approval panel, recommended profile, provider playbook도 반환합니다.
- `tb_profile` / `tb_profile_set`은 supervised session의 policy profile을 직접 조회/조정합니다.
- `tb_approve`는 daemon session memory에 승인 상태를 저장하고 supervised profile 승격을 도울 수 있습니다.
- `tb_telemetry_summary` / `tb_telemetry_recent`는 serve가 기록한 pilot telemetry를 그대로 노출합니다.
- bridge는 secret input을 daemon session memory에만 저장하며 tool 응답에는 raw secret을 노출하지 않습니다.
- bridge는 현재 `touch-browser serve`를 내부 child process로 띄우는 thin proxy이며, serve child에는 `TOUCH_BROWSER_TELEMETRY_SURFACE=mcp`를 주입합니다.
