import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createCronJob,
  cronJobTransactionKey,
  getCronJobForName,
  init as initCron,
} from "@helium/cron-sdk";
import {
  compileTransaction,
  init as initTuktuk,
  taskQueueAuthorityKey,
} from "@helium/tuktuk-sdk";
import { sendInstructions } from "@helium/spl-utils";
import {
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { SolanaGptOracleScheduler } from "../target/types/solana_gpt_oracle_scheduler";

const ORACLE_PROGRAM_ID = new PublicKey(
  "LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab"
);

async function main() {
  const argv = await yargs(hideBin(process.argv))
    .options({
      cronName: {
        type: "string",
        description: "Cron job name to create or reuse",
        demandOption: true,
      },
      taskQueue: {
        type: "string",
        description: "TukTuk task queue public key",
        demandOption: true,
      },
      walletPath: {
        type: "string",
        description: "Path to the wallet keypair",
        demandOption: true,
      },
      rpcUrl: {
        type: "string",
        description: "Solana RPC URL used for display and close commands",
        demandOption: true,
      },
      schedule: {
        type: "string",
        description: "Cron schedule for the GPT request",
        default: "0 * * * * *",
      },
      fundingAmount: {
        type: "number",
        description: "Lamports to fund the cron job with",
        default: 0.01 * LAMPORTS_PER_SOL,
      },
    })
    .help()
    .alias("help", "h").argv;

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const wallet = provider.wallet as anchor.Wallet;
  const program = anchor.workspace
    .solanaGptOracleScheduler as Program<SolanaGptOracleScheduler>;
  const tuktukProgram = await initTuktuk(provider);
  const cronProgram = await initCron(provider);

  const taskQueue = new PublicKey(argv.taskQueue);

  const [oracleState] = PublicKey.findProgramAddressSync(
    [Buffer.from("oracle_state")],
    program.programId
  );
  const [treasury] = PublicKey.findProgramAddressSync(
    [Buffer.from("treasury")],
    program.programId
  );

  const oracleStateAccount = await program.account.oracleState.fetch(
    oracleState
  );
  const llmContext = oracleStateAccount.llmContext as PublicKey;
  if (llmContext.equals(PublicKey.default)) {
    throw new Error(
      "oracle_state.llm_context is unset — call create_context first."
    );
  }

  const [interaction] = PublicKey.findProgramAddressSync(
    [Buffer.from("interaction"), treasury.toBuffer(), llmContext.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  const taskQueueAuthority = taskQueueAuthorityKey(
    taskQueue,
    wallet.publicKey
  )[0];

  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("Program:", program.programId.toBase58());
  console.log("Task queue:", taskQueue.toBase58());
  console.log("Oracle state:", oracleState.toBase58());
  console.log("Treasury:", treasury.toBase58());
  console.log("LLM context:", llmContext.toBase58());
  console.log("Interaction PDA:", interaction.toBase58());
  console.log(
    "Configured tq authority:",
    oracleStateAccount.taskQueueAuthority.toBase58()
  );

  const taskQueueAuthorityInfo = await provider.connection.getAccountInfo(
    taskQueueAuthority
  );
  if (!taskQueueAuthorityInfo) {
    console.log("Initializing wallet queue authority...");
    await tuktukProgram.methods
      .addQueueAuthorityV0()
      .accountsPartial({
        payer: wallet.publicKey,
        queueAuthority: wallet.publicKey,
        taskQueue,
      })
      .rpc({ skipPreflight: true });
  }

  let cronJob = await getCronJobForName(cronProgram, argv.cronName);

  if (!cronJob) {
    const {
      pubkeys: { cronJob: cronJobPubkey },
    } = await (
      await createCronJob(cronProgram, {
        tuktukProgram,
        taskQueue,
        args: {
          name: argv.cronName,
          schedule: argv.schedule,
          freeTasksPerTransaction: 1,
          numTasksPerQueueCall: 1,
        },
      })
    ).rpcAndKeys({ skipPreflight: false });

    cronJob = cronJobPubkey;

    await sendInstructions(provider, [
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: cronJob,
        lamports: argv.fundingAmount,
      }),
    ]);

    const requestGptIx = new TransactionInstruction({
      keys: [
        { pubkey: oracleState, isSigner: false, isWritable: true },
        { pubkey: treasury, isSigner: false, isWritable: true },
        { pubkey: interaction, isSigner: false, isWritable: true },
        { pubkey: llmContext, isSigner: false, isWritable: true },
        {
          pubkey: oracleStateAccount.taskQueueAuthority as PublicKey,
          isSigner: true,
          isWritable: false,
        },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: ORACLE_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      data: program.coder.instruction.encode("requestGpt", {}),
      programId: program.programId,
    });

    const { transaction, remainingAccounts } = compileTransaction(
      [requestGptIx],
      []
    );

    await cronProgram.methods
      .addCronTransactionV0({
        index: 0,
        transactionSource: {
          compiledV0: [transaction],
        },
      })
      .accountsPartial({
        payer: wallet.publicKey,
        cronJob,
        cronJobTransaction: cronJobTransactionKey(cronJob, 0)[0],
      })
      .remainingAccounts(remainingAccounts)
      .rpc({ skipPreflight: true });
  }

  console.log("Cron job:", cronJob.toBase58());
  console.log(`request_gpt scheduled with schedule "${argv.schedule}".`);
  console.log(
    `Close transaction: tuktuk -u ${argv.rpcUrl} -w ${argv.walletPath} cron-transaction close --cron-name ${argv.cronName} --id 0`
  );
  console.log(
    `Close cron: tuktuk -u ${argv.rpcUrl} -w ${argv.walletPath} cron close --cron-name ${argv.cronName}`
  );
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
