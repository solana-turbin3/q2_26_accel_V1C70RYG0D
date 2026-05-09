import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { AnchorEscrowQ22026 } from "../target/types/anchor_escrow_q2_2026";
import {
  Commitment,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import NodeWallet from "@anchor-lang/core/dist/cjs/nodewallet";
import { BN } from "bn.js";
import { randomBytes } from "crypto";
import { expect } from "chai";

const commitment: Commitment = "confirmed";

describe("anchor-escrow-q2-2026", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .anchorEscrowQ22026 as Program<AnchorEscrowQ22026>;
  const connection = provider.connection;
  const payer = provider.wallet as NodeWallet;
  const taker = Keypair.generate();

  let mintA: PublicKey;
  let mintB: PublicKey;
  let makerAtaA: PublicKey;
  let makerAtaB: PublicKey;
  let takerAtaA: PublicKey;
  let takerAtaB: PublicKey;

  const confirmTx = async (signature: string) => {
    const latestBlockhash = await connection.getLatestBlockhash(commitment);
    await connection.confirmTransaction(
      {
        signature,
        ...latestBlockhash,
      },
      commitment
    );
  };

  const confirmTxs = async (signatures: string[]) => {
    await Promise.all(signatures.map(confirmTx));
  };

  const newSeed = () => new BN(randomBytes(8));

  const deriveEscrow = (seed: BN) =>
    PublicKey.findProgramAddressSync(
      [
        Buffer.from("escrow"),
        payer.publicKey.toBuffer(),
        seed.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    )[0];

  const getTokenAmount = async (address: PublicKey) => {
    const account = await getAccount(connection, address, commitment);
    return Number(account.amount);
  };

  const expectClosed = async (address: PublicKey) => {
    expect(await connection.getAccountInfo(address, commitment)).to.be.null;
  };

  const createEscrow = async (depositAmount: number, receiveAmount: number) => {
    const seed = newSeed();
    const escrow = deriveEscrow(seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);
    const makerAtaABefore = await getTokenAmount(makerAtaA);

    const tx = await program.methods
      .make(seed, new BN(depositAmount), new BN(receiveAmount))
      .accountsStrict({
        maker: payer.publicKey,
        mintA,
        mintB,
        makerAtaA,
        escrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await confirmTx(tx);

    const vaultAccount = await getAccount(connection, vault, commitment);
    expect(Number(vaultAccount.amount)).to.equal(depositAmount);
    expect(vaultAccount.owner.toBase58()).to.equal(escrow.toBase58());
    expect(vaultAccount.mint.toBase58()).to.equal(mintA.toBase58());
    expect(await getTokenAmount(makerAtaA)).to.equal(
      makerAtaABefore - depositAmount
    );

    const escrowAccount = await program.account.escrow.fetch(escrow);
    expect(escrowAccount.seed.toString()).to.equal(seed.toString());
    expect(escrowAccount.maker.toBase58()).to.equal(payer.publicKey.toBase58());
    expect(escrowAccount.mintA.toBase58()).to.equal(mintA.toBase58());
    expect(escrowAccount.mintB.toBase58()).to.equal(mintB.toBase58());
    expect(escrowAccount.receive.toNumber()).to.equal(receiveAmount);

    return { escrow, vault, depositAmount, receiveAmount };
  };

  before(async () => {
    await Promise.all([
      connection.requestAirdrop(payer.publicKey, 10 * LAMPORTS_PER_SOL),
      connection.requestAirdrop(taker.publicKey, 10 * LAMPORTS_PER_SOL),
    ]).then(confirmTxs);

    mintA = await createMint(
      connection,
      payer.payer,
      payer.publicKey,
      payer.publicKey,
      6
    );
    mintB = await createMint(
      connection,
      payer.payer,
      payer.publicKey,
      payer.publicKey,
      6
    );

    makerAtaA = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        payer.payer,
        mintA,
        payer.publicKey
      )
    ).address;
    makerAtaB = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        payer.payer,
        mintB,
        payer.publicKey
      )
    ).address;
    takerAtaA = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        payer.payer,
        mintA,
        taker.publicKey
      )
    ).address;
    takerAtaB = (
      await getOrCreateAssociatedTokenAccount(
        connection,
        payer.payer,
        mintB,
        taker.publicKey
      )
    ).address;

    await confirmTxs([
      await mintTo(
        connection,
        payer.payer,
        mintA,
        makerAtaA,
        payer.payer,
        1_000_000_000
      ),
      await mintTo(
        connection,
        payer.payer,
        mintB,
        takerAtaB,
        payer.payer,
        1_000_000_000
      ),
    ]);
  });

  it("makes an escrow and moves the maker deposit into the vault", async () => {
    await createEscrow(1_000_000, 2_000_000);
  });

  it("refunds the maker and closes escrow accounts", async () => {
    const escrowFixture = await createEscrow(3_000_000, 4_000_000);
    const makerAtaABefore = await getTokenAmount(makerAtaA);

    const tx = await program.methods
      .refund()
      .accountsStrict({
        maker: payer.publicKey,
        mintA,
        makerAtaA,
        vault: escrowFixture.vault,
        escrow: escrowFixture.escrow,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await confirmTx(tx);

    expect(await getTokenAmount(makerAtaA)).to.equal(
      makerAtaABefore + escrowFixture.depositAmount
    );
    await expectClosed(escrowFixture.vault);
    await expectClosed(escrowFixture.escrow);
  });

  it("takes the escrow offer and settles both sides of the trade", async () => {
    const escrowFixture = await createEscrow(5_000_000, 6_000_000);
    const takerAtaABefore = await getTokenAmount(takerAtaA);
    const takerAtaBBefore = await getTokenAmount(takerAtaB);
    const makerAtaBBefore = await getTokenAmount(makerAtaB);

    const tx = await program.methods
      .take()
      .accountsStrict({
        taker: taker.publicKey,
        maker: payer.publicKey,
        mintA,
        mintB,
        vault: escrowFixture.vault,
        makerAtaB,
        takerAtaA,
        takerAtaB,
        escrow: escrowFixture.escrow,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([taker])
      .rpc();

    await confirmTx(tx);

    expect(await getTokenAmount(takerAtaA)).to.equal(
      takerAtaABefore + escrowFixture.depositAmount
    );
    expect(await getTokenAmount(takerAtaB)).to.equal(
      takerAtaBBefore - escrowFixture.receiveAmount
    );
    expect(await getTokenAmount(makerAtaB)).to.equal(
      makerAtaBBefore + escrowFixture.receiveAmount
    );
    await expectClosed(escrowFixture.vault);
    await expectClosed(escrowFixture.escrow);
  });
});
