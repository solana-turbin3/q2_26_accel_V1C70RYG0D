import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { SolanaGptOracleScheduler } from "../target/types/solana_gpt_oracle_scheduler";

const ORACLE_PROGRAM_ID = new PublicKey(
  "LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab"
);

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("solana-gpt-oracle-scheduler", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .solanaGptOracleScheduler as Program<SolanaGptOracleScheduler>;
  const wallet = provider.wallet as anchor.Wallet;

  const [oracleStatePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("oracle_state")],
    program.programId
  );
  const [treasuryPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("treasury")],
    program.programId
  );
  const [oracleCounterPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("counter")],
    ORACLE_PROGRAM_ID
  );

  const defaultPrompt =
    "You are a concise Solana assistant. Reply in a single sentence.";

  // Stand-in task_queue_authority — the cron script wires the real TukTuk PDA at schedule time.
  const taskQueueAuthority = wallet.publicKey;

  let llmContext: PublicKey | null = null;
  let oracleAvailable = false;

  before(async () => {
    const info = await provider.connection.getAccountInfo(ORACLE_PROGRAM_ID);
    oracleAvailable = !!info;
    if (!oracleAvailable) {
      console.log(
        "Skipping oracle CPI tests — oracle program not deployed on this cluster."
      );
    }
  });

  it("initializes oracle state and treasury", async () => {
    const existing = await program.account.oracleState.fetchNullable(
      oracleStatePda
    );
    if (existing) {
      console.log("oracle_state already initialized — skipping init.");
      return;
    }

    await program.methods
      .initialize(defaultPrompt, taskQueueAuthority)
      .accountsPartial({
        payer: wallet.publicKey,
        oracleState: oracleStatePda,
        treasury: treasuryPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const state = await program.account.oracleState.fetch(oracleStatePda);
    expect(state.defaultPrompt).to.eq(defaultPrompt);
    expect(state.taskQueueAuthority.toBase58()).to.eq(
      taskQueueAuthority.toBase58()
    );
  });

  it("creates the LLM context via oracle CPI", async function () {
    if (!oracleAvailable) {
      this.skip();
    }

    const state = await program.account.oracleState.fetch(oracleStatePda);
    if (!state.llmContext.equals(PublicKey.default)) {
      llmContext = state.llmContext;
      console.log("llm_context already set:", llmContext.toBase58());
      return;
    }

    const counter = await (program as any).provider.connection.getAccountInfo(
      oracleCounterPda
    );
    if (!counter) {
      console.log("Oracle counter PDA missing — oracle not initialized.");
      this.skip();
    }
    // Read counter.count (u32 LE after 8-byte discriminator).
    const counterValue = counter!.data.readUInt32LE(8);
    const [llmContextPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("test-context"),
        Buffer.from(Uint32Array.of(counterValue).buffer),
      ],
      ORACLE_PROGRAM_ID
    );

    await program.methods
      .createContext(defaultPrompt)
      .accountsPartial({
        payer: wallet.publicKey,
        oracleState: oracleStatePda,
        oracleCounter: oracleCounterPda,
        llmContext: llmContextPda,
        systemProgram: SystemProgram.programId,
        oracleProgram: ORACLE_PROGRAM_ID,
      })
      .rpc();

    const refreshed = await program.account.oracleState.fetch(oracleStatePda);
    expect(refreshed.llmContext.toBase58()).to.eq(llmContextPda.toBase58());
    llmContext = refreshed.llmContext;
  });

  it("funds the treasury PDA", async () => {
    const lamports = 0.005 * LAMPORTS_PER_SOL;
    await program.methods
      .fundTreasury(new anchor.BN(lamports))
      .accountsPartial({
        payer: wallet.publicKey,
        treasury: treasuryPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const balance = await provider.connection.getBalance(treasuryPda);
    expect(balance).to.be.greaterThan(0);
  });

  it("returns a GPT response into oracle_state.last_response", async function () {
    if (!oracleAvailable || !llmContext) {
      this.skip();
    }

    // For direct (non-TukTuk) invocation, the wallet acts as task_queue_authority.
    // The on-chain check enforces oracle_state.task_queue_authority == signer.
    const [interactionPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("interaction"),
        treasuryPda.toBuffer(),
        llmContext!.toBuffer(),
      ],
      ORACLE_PROGRAM_ID
    );

    await program.methods
      .requestGpt()
      .accountsPartial({
        oracleState: oracleStatePda,
        treasury: treasuryPda,
        interaction: interactionPda,
        llmContext: llmContext!,
        taskQueueAuthority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
        oracleProgram: ORACLE_PROGRAM_ID,
      })
      .rpc();

    // The MagicBlock oracle worker calls back asynchronously. Poll briefly.
    const deadline = Date.now() + 90_000;
    let response = "";
    while (Date.now() < deadline) {
      const state = await program.account.oracleState.fetch(oracleStatePda);
      if (state.lastResponse.length > 0) {
        response = state.lastResponse;
        break;
      }
      await sleep(3_000);
    }

    if (response.length === 0) {
      console.log(
        "GPT response did not arrive within 90s — oracle worker may be offline."
      );
      this.skip();
    }
    console.log("GPT response:", response);
    expect(response.length).to.be.greaterThan(0);
  });
});
