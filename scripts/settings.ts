import { AnchorProvider, Wallet } from "@coral-xyz/anchor";
import {
    Connection,
    Keypair,
    sendAndConfirmTransaction,
    Transaction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import idl from "../target/idl/charcoin.json";
import bs58 from "bs58";
import { BN } from "bn.js";
import { Charcoin } from "../target/types/charcoin";
import fs from "fs"
import path from "path"
import { homedir } from "os";
import { getOrCreateAssociatedTokenAccount, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import { ASSOCIATED_PROGRAM_ID } from "@coral-xyz/anchor/dist/cjs/utils/token";
// Replace with your mainnet RPC URL
const RPC_URL = "https://api.devnet.solana.com";

// Retrieve your plain private key from an environment variable.
// The PRIVATE_KEY should be a string (for example, a base58-encoded key)
const privateKeyArray = JSON.parse(fs.readFileSync(path.join(homedir(),".config/solana/id.json"), 'utf8'));
// Convert to Uint8Array
const privateKeyUint8Array = new Uint8Array(privateKeyArray);

// Generate Keypair
const keypair = Keypair.fromSecretKey(privateKeyUint8Array);

console.log("Public Key:", keypair.publicKey.toBase58());

async function main() {
    // Create a connection to the mainnet
    const connection = new Connection(RPC_URL, "confirmed");

    // Create a wallet instance from your keypair
    const admin = new Wallet(keypair);

    // Create the Anchor provider using the connection and wallet
    const provider = new AnchorProvider(connection, admin, {
        preflightCommitment: "confirmed",
    });


    // Initialize the program using your IDL and provider
    const program = new anchor.Program<Charcoin>(idl as Charcoin, provider);

    console.log(
        "Program initialized on mainnet. Program ID:",
        program.programId.toString()
        // program
    );

    try {
      
    
 const [configAccount] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from('config')],
    program.programId
  );
 
  const mint= new anchor.web3.PublicKey("chAZFTpRrSj4nbygm5ZgqoPD5GffDwMCv4iKXhZ2X9f")
  
  const[stakingPool] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from('staking_pool'), mint.toBuffer()],
      program.programId
    );
 
    const [stakingRewardAccount] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from('staking_reward'),mint.toBuffer()],
      program.programId
    );

const  stakingPoolAta = await getOrCreateAssociatedTokenAccount(
          program.provider.connection,
          admin.payer,
          mint,
          stakingPool,
          true,
          null,
          null,
          TOKEN_2022_PROGRAM_ID,
          ASSOCIATED_PROGRAM_ID,
        );
console.log("Staking Pool ATA:", stakingPoolAta.address.toBase58());
    // Add your test here.
    const configIx =   await program.methods
          .setRewardPercentageHandler(
                // reward       , lockup          ,   vote power      
                    new anchor.BN(50),new anchor.BN(1),new anchor.BN(500),new anchor.BN(100), //  5 , 1, 0.5 
                    new anchor.BN(70),new anchor.BN(90),new anchor.BN(1000),new anchor.BN(100), // 7, 90, 1
                    new anchor.BN(150),new anchor.BN(180),new anchor.BN(3000),new anchor.BN(100),  // 15, 180, 3
                    new anchor.BN(180),new anchor.BN(180),new anchor.BN(3000),new anchor.BN(100),  // 15, 180, 3
          )
        .accounts({
        stakingPool: stakingPool,
                configAccount: configAccount,
                admin:admin.publicKey

      })
    .instruction();

 const updateSettingTx = await program.methods
      .updateSettings(
        new anchor.BN(10*1e6),// min_governance_stake = 10 token
        new anchor.BN(1*86400), // min_stake_duration_voting = 1 day
      )
      .accounts({
                config: configAccount,
                admin:admin.publicKey

      })
          .instruction();


            const tx = new Transaction()
            
            tx.add(configIx);
            tx.add(updateSettingTx);

            tx.feePayer = admin.publicKey;
            tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

            // console.log("Transaction:", tx);
            const signedTx = await admin.signTransaction(tx);

            const simulateResult = await connection.simulateTransaction(signedTx);
            console.log("Simulate result: ", simulateResult);

            const txId = await sendAndConfirmTransaction(connection, signedTx, [keypair]);
            console.log("txId ", txId);
    } catch (error) {
        console.error("Error fetching fee accounts:", error);
    }
}

main().catch((error) => {
    console.error("Error in main():", error);
});
