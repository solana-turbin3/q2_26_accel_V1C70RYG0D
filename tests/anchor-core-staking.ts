import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorCoreStaking } from "../target/types/anchor_core_staking";
import { SystemProgram } from "@solana/web3.js";
import { MPL_CORE_PROGRAM_ID, fetchAsset, fetchCollection } from "@metaplex-foundation/mpl-core";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { publicKey } from "@metaplex-foundation/umi";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

const MILLISECONDS_PER_DAY = 86400000;
const SECONDS_PER_DAY = 86400;
const REWARDS_BPS = 10000; // with 6 decimals => 1 reward token per staked day
const FREEZE_PERIOD_IN_DAYS = 7;

describe("anchor-core-staking", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.anchorCoreStaking as Program<AnchorCoreStaking>;

  // Read-only umi client used to inspect on-chain Core asset/collection attributes.
  const umi = createUmi(provider.connection.rpcEndpoint);

  // Generate a keypair for the collection
  const collectionKeypair = anchor.web3.Keypair.generate();

  // Find the update authority for the collection (PDA)
  const updateAuthority = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("update_authority"), collectionKeypair.publicKey.toBuffer()],
    program.programId
  )[0];

  // Generate a keypair for the nft asset
  const nftKeypair = anchor.web3.Keypair.generate();

  // Find the config account (PDA)
  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config"), collectionKeypair.publicKey.toBuffer()],
    program.programId
  )[0];

  // Find the rewards mint account (PDA)
  const rewardsMint = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("rewards_mint"), config.toBuffer()],
    program.programId
  )[0];

  const userRewardsAta = getAssociatedTokenAddressSync(
    rewardsMint,
    provider.wallet.publicKey,
    false,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  // ----- Helpers -----

  // Advance the surfnet clock. `absoluteTimestamp` is in milliseconds.
  async function advanceTime(absoluteTimestampMs: number): Promise<void> {
    const rpcResponse = await fetch(provider.connection.rpcEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "surfnet_timeTravel",
        params: [{ absoluteTimestamp: absoluteTimestampMs }],
      }),
    });
    const result = (await rpcResponse.json()) as { error?: any; result?: any };
    if (result.error) {
      throw new Error(`Time travel failed: ${JSON.stringify(result.error)}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  // Read the on-chain unix timestamp (seconds) from the Clock sysvar.
  async function getOnChainTimestamp(): Promise<number> {
    const info = await provider.connection.getAccountInfo(
      anchor.web3.SYSVAR_CLOCK_PUBKEY
    );
    // Clock layout: slot(u64), epoch_start_ts(i64), epoch(u64), lse(u64), unix_timestamp(i64)
    return Number(info!.data.readBigInt64LE(32));
  }

  function attrMap(attributeList?: { key: string; value: string }[]): Record<string, string> {
    const map: Record<string, string> = {};
    (attributeList ?? []).forEach((a) => (map[a.key] = a.value));
    return map;
  }

  async function getAssetAttrs(): Promise<Record<string, string>> {
    const asset = await fetchAsset(umi, publicKey(nftKeypair.publicKey.toBase58()));
    return attrMap(asset.attributes?.attributeList as any);
  }

  async function getCollectionAttrs(): Promise<Record<string, string>> {
    const collection = await fetchCollection(
      umi,
      publicKey(collectionKeypair.publicKey.toBase58())
    );
    return attrMap(collection.attributes?.attributeList as any);
  }

  async function getRewardUiBalance(): Promise<number> {
    try {
      const bal = await provider.connection.getTokenAccountBalance(userRewardsAta);
      return bal.value.uiAmount ?? 0;
    } catch {
      return 0;
    }
  }

  async function claimRewards() {
    return program.methods
      .claimRewards()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        config,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        rewardsMint,
        userRewardsAta,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .rpc();
  }

  async function stake() {
    return program.methods
      .stake()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .rpc();
  }

  async function unstake() {
    return program.methods
      .unstake()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        userRewardsAta,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();
  }

  // staked_at recorded at stake time, reused across tests.
  let stakedAt = 0;

  it("Create a collection", async () => {
    const tx = await program.methods
      .createCollection("Test Collection", "https://example.com/collection")
      .accountsPartial({
        payer: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([collectionKeypair])
      .rpc();
    console.log("\nCollection created:", collectionKeypair.publicKey.toBase58(), tx);
  });

  it("Mint an NFT", async () => {
    const tx = await program.methods
      .mintAsset("Test NFT", "https://example.com/nft")
      .accountsPartial({
        user: provider.wallet.publicKey,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([nftKeypair])
      .rpc();
    console.log("\nNFT minted:", nftKeypair.publicKey.toBase58(), tx);
  });

  it("Initialize Config", async () => {
    await program.methods
      .initialize(REWARDS_BPS, FREEZE_PERIOD_IN_DAYS)
      .accountsPartial({
        admin: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
    console.log("\nConfig initialized. rewards_bps:", REWARDS_BPS, "freeze:", FREEZE_PERIOD_IN_DAYS);
  });

  it("Stake an NFT (initializes last_claim and collection stats)", async () => {
    await program.methods
      .stake()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .rpc();

    const attrs = await getAssetAttrs();
    stakedAt = parseInt(attrs["staked_at"], 10);
    assert.equal(attrs["staked"], "true", "asset should be staked");
    assert.isAbove(stakedAt, 0, "staked_at should be set");
    assert.equal(attrs["last_claim"], attrs["staked_at"], "last_claim should start equal to staked_at");

    // Challenge 2: collection statistics
    const coll = await getCollectionAttrs();
    assert.equal(coll["total_staked"], "1", "collection total_staked should be 1");
    assert.equal(coll["total_staked_lifetime"], "1", "collection lifetime should be 1");
    console.log("\nStaked. staked_at:", stakedAt, "collection:", coll);
  });

  it("Claim rewards mid-stake WITHOUT resetting the freeze clock", async () => {
    // Travel ~3 days past staking (+1h buffer to avoid day-boundary truncation).
    await advanceTime(stakedAt * 1000 + 3 * MILLISECONDS_PER_DAY + 3600000);

    const before = await getRewardUiBalance();
    await claimRewards();
    const after = await getRewardUiBalance();

    assert.equal(after - before, 3, "should mint exactly 3 reward tokens (3 days)");

    const attrs = await getAssetAttrs();
    // Freeze clock untouched, reward clock advanced, still staked.
    assert.equal(parseInt(attrs["staked_at"], 10), stakedAt, "staked_at (freeze clock) must NOT change on claim");
    assert.equal(attrs["staked"], "true", "asset must remain staked after claim");
    assert.equal(
      parseInt(attrs["last_claim"], 10),
      stakedAt + 3 * SECONDS_PER_DAY,
      "last_claim should advance by exactly 3 days"
    );

    // Claiming does not change the collection staked count.
    const coll = await getCollectionAttrs();
    assert.equal(coll["total_staked"], "1", "claim must not change total_staked");
    console.log("\nClaimed 3 tokens. last_claim advanced, staked_at intact.");
  });

  it("Unstake still fails before the freeze period (proves claim kept the freeze clock)", async () => {
    // We are ~3 days in; freeze period is 7 days. Even though we just claimed,
    // unstaking must still be rejected because staked_at was preserved.
    try {
      await unstake();
      throw new Error("Unstake should have failed before the freeze period elapsed");
    } catch (err) {
      if (err instanceof anchor.AnchorError && err.error.errorCode.code === "FreezePeriodNotElapsed") {
        console.log("\nUnstake correctly rejected:", err.error.errorMessage);
      } else {
        throw err;
      }
    }
  });

  it("Unstake after the freeze period pays only the un-claimed remainder (no double counting)", async () => {
    // Travel to ~8 days past staking (+1h buffer).
    await advanceTime(stakedAt * 1000 + 8 * MILLISECONDS_PER_DAY + 3600000);

    const before = await getRewardUiBalance();
    await unstake();
    const after = await getRewardUiBalance();

    // Already claimed 3 days; 8 - 3 = 5 days remain to be paid on unstake.
    assert.equal(after - before, 5, "unstake should mint the remaining 5 days");
    // Total rewards over the lifecycle equal the full 8 staked days (no loss, no double pay).
    assert.equal(after, 8, "total rewards must equal the 8 staked days");

    const attrs = await getAssetAttrs();
    assert.equal(attrs["staked"], "false", "asset should be unstaked");

    // Challenge 2: collection stats decremented; lifetime preserved.
    const coll = await getCollectionAttrs();
    assert.equal(coll["total_staked"], "0", "collection total_staked should be 0 after unstake");
    assert.equal(coll["total_staked_lifetime"], "1", "collection lifetime should remain 1");
    console.log("\nUnstaked. Total rewards:", after, "collection:", coll);
  });

  it("Re-stake the same NFT after unstaking (exercises the FreezeDelegate update path)", async () => {
    await stake();

    const attrs = await getAssetAttrs();
    assert.equal(attrs["staked"], "true", "asset should be staked again");
    assert.equal(attrs["last_claim"], attrs["staked_at"], "reward clock should reset to the new stake time");
    assert.isAbove(parseInt(attrs["staked_at"], 10), stakedAt, "new staked_at should be later than the first stake");

    // Current count back to 1; lifetime counts the second stake event.
    const coll = await getCollectionAttrs();
    assert.equal(coll["total_staked"], "1", "collection total_staked should be 1 after re-stake");
    assert.equal(coll["total_staked_lifetime"], "2", "collection lifetime should be 2 after the second stake");
    console.log("\nRe-staked successfully. collection:", coll);
  });
});
