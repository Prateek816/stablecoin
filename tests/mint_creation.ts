
import * as anchor from "@coral-xyz/anchor";

import {
  TOKEN_2022_PROGRAM_ID,
  AccountState,
  getMint,
  getTransferFeeConfig,
  getMetadataPointerState,
  getDefaultAccountState,
  getMintCloseAuthority,
  getTokenMetadata,
} from "@solana/spl-token";

import {
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";

import { assert } from "chai";

describe("RUSD V1 - Mint Creation", () => {

  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);

  const program = anchor.workspace.RemittanceStablecoin;

  const issuer = provider.wallet.publicKey;

  const [mint] = PublicKey.findProgramAddressSync(
    [Buffer.from("rusd-mint")],
    program.programId
  );

  // ------------------------------------------------
  // TEST 1: CREATE MINT
  // ------------------------------------------------

  it("creates the RUSD V1 mint", async () => {

    const tx = await program.methods

      .createV1Mint({
        name: "Remittance USD",
        symbol: "RUSD",
        uri: "https://example.com/rusd.json",
      })

      .accounts({
        issuer,
        mint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })

      .rpc();

    console.log("Creation transaction:", tx);
    console.log("Mint address:", mint.toBase58());

    assert.isNotEmpty(tx);
  });

  // ------------------------------------------------
  // TEST 2: TOKEN-2022 OWNERSHIP
  // ------------------------------------------------

  it("is owned by Token-2022", async () => {

    const accountInfo =
      await provider.connection.getAccountInfo(mint);

    assert.isNotNull(accountInfo);

    assert.isTrue(
      accountInfo!.owner.equals(TOKEN_2022_PROGRAM_ID)
    );
  });

  // ------------------------------------------------
  // TEST 3: BASE MINT CONFIGURATION
  // ------------------------------------------------

  it("has the correct decimals and authorities", async () => {

    const mintState = await getMint(
      provider.connection,
      mint,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    assert.equal(mintState.decimals, 6);

    assert.equal(mintState.supply, 0n);

    assert.isTrue(
      mintState.mintAuthority!.equals(issuer)
    );

    assert.isTrue(
      mintState.freezeAuthority!.equals(issuer)
    );
  });

  // ------------------------------------------------
  // TEST 4: TRANSFER FEE CONFIGURATION
  // ------------------------------------------------

  it("has a 50 bps transfer fee", async () => {

    const mintState = await getMint(
      provider.connection,
      mint,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    const feeConfig = getTransferFeeConfig(mintState);

    assert.isNotNull(feeConfig);

    assert.equal(
      feeConfig!.newerTransferFee.transferFeeBasisPoints,
      50
    );

    assert.equal(
      feeConfig!.newerTransferFee.maximumFee,
      10_000_000n
    );

    assert.isTrue(
      feeConfig!.transferFeeConfigAuthority!.equals(issuer)
    );

    assert.isTrue(
      feeConfig!.withdrawWithheldAuthority!.equals(issuer)
    );
  });

  // ------------------------------------------------
  // TEST 5: METADATA POINTER
  // ------------------------------------------------

  it("points metadata to the mint itself", async () => {

    const mintState = await getMint(
      provider.connection,
      mint,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    const pointer = getMetadataPointerState(mintState);

    assert.isNotNull(pointer);

    assert.isTrue(
      pointer!.metadataAddress!.equals(mint)
    );

    assert.isTrue(
      pointer!.authority!.equals(issuer)
    );
  });

  // ------------------------------------------------
  // TEST 6: DEFAULT ACCOUNT STATE
  // ------------------------------------------------

  it("sets new token accounts to Frozen", async () => {

    const mintState = await getMint(
      provider.connection,
      mint,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    const defaultState = getDefaultAccountState(mintState);

    assert.isNotNull(defaultState);

    assert.equal(
      defaultState!.state,
      AccountState.Frozen
    );
  });

  // ------------------------------------------------
  // TEST 7: MINT CLOSE AUTHORITY
  // ------------------------------------------------

  it("sets the issuer as mint close authority", async () => {

    const mintState = await getMint(
      provider.connection,
      mint,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    const closeAuthority = getMintCloseAuthority(mintState);

    assert.isNotNull(closeAuthority);

    assert.isTrue(
      closeAuthority!.closeAuthority!.equals(issuer)
    );
  });

  // ------------------------------------------------
  // TEST 8: TOKEN METADATA
  // ------------------------------------------------

  it("stores metadata directly on the mint", async () => {

    const metadata = await getTokenMetadata(
      provider.connection,
      mint,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    assert.isNotNull(metadata);

    assert.equal(metadata!.name, "Remittance USD");

    assert.equal(metadata!.symbol, "RUSD");

    assert.equal(
      metadata!.uri,
      "https://example.com/rusd.json"
    );

    assert.isTrue(
      metadata!.updateAuthority!.equals(issuer)
    );
  });

  // ------------------------------------------------
  // TEST 9: DUPLICATE INITIALIZATION
  // ------------------------------------------------

  it("rejects creating the same mint twice", async () => {

    try {

      await program.methods

        .createV1Mint({
          name: "Duplicate RUSD",
          symbol: "RUSD",
          uri: "https://example.com/duplicate.json",
        })

        .accounts({
          issuer,
          mint,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })

        .rpc();

      assert.fail("Duplicate mint creation should fail");

    } catch (error: any) {

      assert.include(
        error.toString(),
        "MintAlreadyExists"
      );
    }
  });

});
