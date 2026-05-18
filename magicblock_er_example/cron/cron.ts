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
import { ErStateAccount } from "../target/types/er_state_account";

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
      user: {
        type: "string",
        description: "User account owner to schedule updates for",
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
  const program = anchor.workspace.erStateAccount as Program<ErStateAccount>;
  const tuktukProgram = await initTuktuk(provider);
  const cronProgram = await initCron(provider);

  const taskQueue = new PublicKey(argv.taskQueue);
  const user = argv.user ? new PublicKey(argv.user) : wallet.publicKey;
  const userAccount = PublicKey.findProgramAddressSync(
    [Buffer.from("user"), user.toBuffer()],
    program.programId
  )[0];

  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("Program:", program.programId.toBase58());
  console.log("Task queue:", taskQueue.toBase58());
  console.log("User account:", userAccount.toBase58());

  const taskQueueAuthority = taskQueueAuthorityKey(
    taskQueue,
    wallet.publicKey
  )[0];
  const taskQueueAuthorityInfo = await provider.connection.getAccountInfo(
    taskQueueAuthority
  );

  if (!taskQueueAuthorityInfo) {
    console.log("Initializing wallet queue authority...");
    await tuktukProgram.methods
      .addQueueAuthorityV0()
      .accounts({
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
          schedule: "0 * * * * *",
          freeTasksPerTransaction: 0,
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

    const scheduledUpdateIx = new TransactionInstruction({
      keys: [{ pubkey: userAccount, isSigner: false, isWritable: true }],
      data: program.coder.instruction.encode("scheduledUpdate", {}),
      programId: program.programId,
    });

    const { transaction, remainingAccounts } = compileTransaction(
      [scheduledUpdateIx],
      []
    );

    await cronProgram.methods
      .addCronTransactionV0({
        index: 0,
        transactionSource: {
          compiledV0: [transaction],
        },
      })
      .accounts({
        payer: wallet.publicKey,
        cronJob,
        cronJobTransaction: cronJobTransactionKey(cronJob, 0)[0],
      })
      .remainingAccounts(remainingAccounts)
      .rpc({ skipPreflight: true });
  }

  console.log("Cron job:", cronJob.toBase58());
  console.log("scheduled_update will run every minute.");
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
