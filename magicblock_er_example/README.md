# MagicBlock ER VRF Assignment

This assignment extends the MagicBlock ER state account example with VRF-backed state updates and TukTuk automation.

## Program Flow

- `initialize` creates one `UserAccount` PDA at `[b"user", user]`.
- `request_random_update` requests MagicBlock VRF on the base layer using `DEFAULT_QUEUE`.
- `request_random_update_er` requests MagicBlock VRF inside the Ephemeral Rollup using `DEFAULT_EPHEMERAL_QUEUE`.
- `consume_random_update` is the verified VRF callback. It requires `VRF_PROGRAM_IDENTITY` as signer and writes the first 8 bytes of the VRF randomness into `UserAccount.data`.
- Existing `delegate` and `undelegate` instructions move the same user PDA into and out of the ER.
- `scheduled_update` is a permissionless endpoint that increments `UserAccount.data`, designed for TukTuk crankers.
- `schedule_tuktuk_update` queues a one-off TukTuk task that runs `scheduled_update` immediately.

## TukTuk Cron

The cron script creates a recurring TukTuk cron job that runs `scheduled_update` every minute:

```sh
anchor run cron
```

For a custom queue or user account owner:

```sh
yarn ts-node cron/cron.ts \
	--cronName magicblock-user-data-cron \
	--taskQueue <TASK_QUEUE_PUBKEY> \
	--walletPath ~/.config/solana/id.json \
	--rpcUrl https://api.devnet.solana.com \
	--user <USER_PUBKEY>
```

The live scheduler test is gated behind `TUKTUK_TASK_QUEUE` because it requires a funded devnet TukTuk task queue with the program's `[b"queue_authority"]` PDA added as an authority.

## Verification

```sh
anchor build
yarn tsc --noEmit
yarn lint
```

The TypeScript test requests randomness once outside the ER, runs the scheduled update endpoint directly, optionally queues a live TukTuk task, delegates the account, requests randomness again inside the ER, then undelegates so the ER state can commit back to the base layer.