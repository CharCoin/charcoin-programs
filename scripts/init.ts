import { AnchorProvider, Wallet } from "@coral-xyz/anchor";
import {
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    Transaction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import idl from "../target/idl/charcoin.json";
import { Charcoin } from "../target/types/charcoin";
import fs from "fs"
import path from "path"
import { homedir } from "os";
// Replace with your mainnet RPC URL
const RPC_URL = "https://mainnet.helius-rpc.com/?api-key=";

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

 
    const mint= new anchor.web3.PublicKey("charyAhpBstVjf5VnszNiY8UUVDbvA167dQJqpBY2hw")
    const context = {
      user: admin.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
      config: configAccount,
      mint
    } 
    const config = {
      charFunds: new PublicKey("5wagZFtJiZRHNvLzxcLBjUSi7shRP7SEJBHCcML6t6Ez"),
      marketingWallet1: new PublicKey("DuJif6nwgnbqDbxPqmcMD83DyCfwLTRLdrAb5dqpt1xG"),
      marketingWallet2: new PublicKey("B4gjNyPdYJtswDTYT8YjpssCcoGuHEeLz6Aj7JxSijjC"),
      monthlyTopTierWallet:new PublicKey("F2PNU2eYTs5WrtnqQgKn33E2tyh4mcH1RsZgfdVdzmzg"),
      annualTopTierWallet:new PublicKey("EdHfecFGu9iyvDiZj51GHsVvCdAvTpTHVDt3H1f2CLgZ"),
      monthlyCharityLotteryWallet:new PublicKey("93oqwNefboRpzPtb3BiCVYUModCXVdym2gFXoG9cXxT2"),
      annualCharityLotteryWallet:new PublicKey("8ucarmw1tMLhGM1RfUeJgVmbqEZvVDkCrKiErhNNsQ4h"),
      monthlyOneTimeCausesWallet:new PublicKey("FAsAcUPsTAvAD5Nq4gtF6rHm5DhyCTc8M9JGfS2bqyQA"),
      monthlyInfiniteImpactCausesWallet:new PublicKey("D8T2kxgpqfi45QuUeQXg6pC5t3RzNniGFYAEQZycUSZJ"),
      annualOneTimeCausesWallet:new PublicKey("5WPEMTuffpnVwxSiYBTLn3c46PpXEMHiZ8ZjLx9PZkpb"),
      annualInfiniteImpactCausesWallet:new PublicKey("G5r3PmythFbaXp8FmDPRz76VHCHYe9YzzfP5juCPd4nr"),
      deathWallet: new PublicKey("2QJeZJmP15X1ft1jR2Hcucm444e7n3wXgB1wzpYSa3zS"),
      treasuryAuthority: new PublicKey("B3w6t2xkpjMcn1iKkdAxVVAfGXj9CYLFsDVisuqC7WPp"),
      admin: admin.publicKey,
      charTokenMint:mint,
    };
    // Add your test here.
    const configIx =  await program.methods.initialize(
      config
    )        
    .accounts(context)
    .instruction();


            const tx = new Transaction().add(configIx);

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
