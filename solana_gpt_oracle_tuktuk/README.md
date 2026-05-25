# Solana GPT Oracle + TukTuk

This program schedules requests to the
[MagicBlock Solana GPT Oracle](https://github.com/magicblock-labs/super-smart-contracts)
(`LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab`) via the
[TukTuk](https://github.com/helium/tuktuk) cron / task-queue runtime. The agent
response is delivered back to this program through the oracle's identity-signed
callback and persisted in on-chain state.

## Program ID

`HtxZqtUjDbMtkcs3e3BcBroJqXSaVKcyMjPS3QMJP8bd`

## Instructions

| Instruction | Purpose |
|---|---|
| `initialize(default_prompt, task_queue_authority)` | Create the `oracle_state` and system-owned `treasury` PDA. |
| `create_context(agent_description)` | CPI the oracle to create an `LlmContext` PDA and remember it. |
| `fund_treasury(lamports)` | Top up the treasury PDA that pays oracle interaction rent. |
| `request_gpt()` | CPI the oracle's `interact_with_llm`, registering `process_oracle_callback` as the callback. |
| `process_oracle_callback(response)` | Oracle identity-signed callback that stores the GPT response in `oracle_state.last_response`. |
| `schedule(task_id)` | Queue `request_gpt` on a TukTuk task queue using the program's `queue_authority` PDA as the queue authority signer. |

## TukTuk wiring

The companion script `cron/cron.ts` provisions a TukTuk cron job that fires
`request_gpt` on a configurable schedule. Run it through Anchor scripts:

```bash
anchor run cron
```

## End-to-end flow

1. `initialize` — set up state and treasury.
2. `create_context` — define the agent description (CPI to oracle).
3. `fund_treasury` — fund a small treasury for oracle interaction rent.
4. `schedule` — queue the recurring request via TukTuk.
5. TukTuk crank runs `request_gpt` → oracle stores the interaction.
6. MagicBlock GPT oracle worker calls back into this program with the response.
7. `process_oracle_callback` writes the response to `oracle_state.last_response`.
