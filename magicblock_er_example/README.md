# MagicBlock ER VRF Assignment

This assignment extends the MagicBlock ER state account example with VRF-backed state updates.

## Program Flow

- `initialize` creates one `UserAccount` PDA at `[b"user", user]`.
- `request_random_update` requests MagicBlock VRF on the base layer using `DEFAULT_QUEUE`.
- `request_random_update_er` requests MagicBlock VRF inside the Ephemeral Rollup using `DEFAULT_EPHEMERAL_QUEUE`.
- `consume_random_update` is the verified VRF callback. It requires `VRF_PROGRAM_IDENTITY` as signer and writes the first 8 bytes of the VRF randomness into `UserAccount.data`.
- Existing `delegate` and `undelegate` instructions move the same user PDA into and out of the ER.

## Verification

```sh
anchor build
yarn tsc --noEmit
```

The TypeScript test requests randomness once outside the ER, delegates the account, requests randomness again inside the ER, then undelegates so the ER state can commit back to the base layer.