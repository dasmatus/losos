// The hot wallet is a BIP-39 phrase on the MetaMask path
// m/44'/60'/0'/0/0, which is what viem, MetaMask, Rabby and Rainbow derive.
// Anvil's well-known mnemonic is the fixture: account 0 and account 1 must
// come out as anvil's first two addresses, or the wallet is not the one a
// buyer can import into a wallet they already use.
import { english, generateMnemonic, mnemonicToAccount } from "viem/accounts";

const ANVIL = "test test test test test test test test test test test junk";
const ACCOUNT0 = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
const ACCOUNT1 = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";

const first = mnemonicToAccount(ANVIL);
const second = mnemonicToAccount(ANVIL, { addressIndex: 1 });
if (first.address.toLowerCase() !== ACCOUNT0) {
  throw new Error(`account 0 is ${first.address}, not the MetaMask path`);
}
if (second.address.toLowerCase() !== ACCOUNT1) {
  throw new Error(`account 1 is ${second.address}, not the MetaMask path`);
}

// Creating one and importing it back is the same address.
const created = generateMnemonic(english);
const imported = mnemonicToAccount(created);
const again = mnemonicToAccount(created);
if (imported.address !== again.address) {
  throw new Error("importing the new phrase did not reproduce the address");
}

const address = first.address.toLowerCase();
const chain = process.argv[2] ?? "84532";
const message = `LosOS market payout v1\nchain: ${chain}\naddress: ${address}\n`;
const signature = await first.signMessage({ message });
process.stdout.write(JSON.stringify({ address, message, signature }));
