// MiniBlock wallet dashboard.
//
// Security model: the keypair is generated in the browser (WebCrypto P-256)
// and the private key NEVER leaves this page — it is not rendered into the
// DOM, not logged, and not sent to the server. Transfers are signed locally;
// POST /transaction carries only {sender, recipient, amount, public key,
// signature} and the server verifies the signature.
//
// The pure crypto helpers below have no DOM dependencies so they can be
// exercised from node (see module.exports guard at the bottom).

// -- byte helpers ----------------------------------------------------------

function hexEncode(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function hexDecode(hex) {
  if (hex.length % 2 !== 0) throw new Error("odd hex length");
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    const v = parseInt(hex.substr(i * 2, 2), 16);
    if (Number.isNaN(v)) throw new Error("bad hex");
    out[i] = v;
  }
  return out;
}

async function sha256(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return new Uint8Array(digest);
}

// -- RIPEMD-160 (compact implementation; verified against test vectors) -----

function ripemd160(messageBytes) {
  let msg = Array.from(messageBytes);
  const bitLen = msg.length * 8;
  msg.push(0x80);
  while (msg.length % 64 !== 56) msg.push(0x00);
  // 64-bit little-endian bit length (lengths here are tiny; high word is 0).
  const lo = bitLen >>> 0;
  const hi = Math.floor(bitLen / 0x100000000);
  for (const w of [lo, hi]) {
    msg.push(w & 0xff, (w >>> 8) & 0xff, (w >>> 16) & 0xff, (w >>> 24) & 0xff);
  }

  const R1 = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    7, 4, 13, 1, 10, 6, 15, 3, 12, 0, 9, 5, 2, 14, 11, 8,
    3, 10, 14, 4, 9, 15, 8, 1, 2, 7, 0, 6, 13, 11, 5, 12,
    1, 9, 11, 10, 0, 8, 12, 4, 13, 3, 7, 15, 14, 5, 6, 2,
    4, 0, 5, 9, 7, 12, 2, 10, 14, 1, 3, 8, 11, 6, 15, 13,
  ];
  const R2 = [
    5, 14, 7, 0, 9, 2, 11, 4, 13, 6, 15, 8, 1, 10, 3, 12,
    6, 11, 3, 7, 0, 13, 5, 10, 14, 15, 8, 12, 4, 9, 1, 2,
    15, 5, 1, 3, 7, 14, 6, 9, 11, 8, 12, 2, 10, 0, 4, 13,
    8, 6, 4, 1, 3, 11, 15, 0, 5, 12, 2, 13, 9, 7, 10, 14,
    12, 15, 10, 4, 1, 5, 8, 7, 6, 2, 13, 14, 0, 3, 9, 11,
  ];
  const RH1 = [
    11, 14, 15, 12, 5, 8, 7, 9, 11, 13, 14, 15, 6, 7, 9, 8,
    7, 6, 8, 13, 11, 9, 7, 15, 7, 12, 15, 9, 11, 7, 13, 12,
    11, 13, 6, 7, 14, 9, 13, 15, 14, 8, 13, 6, 5, 12, 7, 5,
    11, 12, 14, 15, 14, 15, 9, 8, 9, 14, 5, 6, 8, 6, 5, 12,
    9, 15, 5, 11, 6, 8, 13, 12, 5, 12, 13, 14, 11, 8, 5, 6,
  ];
  const RH2 = [
    8, 9, 9, 11, 13, 15, 15, 5, 7, 7, 8, 11, 14, 14, 12, 6,
    9, 13, 15, 7, 12, 8, 9, 11, 7, 7, 12, 7, 6, 15, 13, 11,
    9, 7, 15, 11, 8, 6, 6, 14, 12, 13, 5, 14, 13, 13, 7, 5,
    15, 5, 8, 11, 14, 14, 6, 14, 6, 9, 12, 9, 12, 5, 15, 8,
    8, 5, 12, 9, 12, 5, 14, 6, 8, 13, 6, 5, 15, 13, 11, 11,
  ];
  const KL = [0x00000000, 0x5a827999, 0x6ed9eba1, 0x8f1bbcdc, 0xa953fd4e];
  const KR = [0x50a28be6, 0x5c4dd124, 0x6d703ef3, 0x7a6d76e9, 0x00000000];

  const rol = (x, n) => ((x << n) | (x >>> (32 - n))) >>> 0;
  const add = (...xs) => xs.reduce((a, b) => (a + b) >>> 0, 0);

  let h0 = 0x67452301, h1 = 0xefcdab89, h2 = 0x98badcfe,
    h3 = 0x10325476, h4 = 0xc3d2e1f0;

  for (let off = 0; off < msg.length; off += 64) {
    const X = [];
    for (let i = 0; i < 16; i++) {
      X.push(
        (msg[off + i * 4] |
          (msg[off + i * 4 + 1] << 8) |
          (msg[off + i * 4 + 2] << 16) |
          (msg[off + i * 4 + 3] << 24)) >>> 0,
      );
    }
    let al = h0, bl = h1, cl = h2, dl = h3, el = h4;
    let ar = h0, br = h1, cr = h2, dr = h3, er = h4;
    for (let j = 0; j < 80; j++) {
      const r = (j / 16) | 0;
      // Left line. The round function takes (b, c, d) — not (a, b, c).
      let fl;
      if (r === 0) fl = (bl ^ cl ^ dl) >>> 0;
      else if (r === 1) fl = ((bl & cl) | (~bl & dl)) >>> 0;
      else if (r === 2) fl = ((bl | ~cl) ^ dl) >>> 0;
      else if (r === 3) fl = ((bl & dl) | (cl & ~dl)) >>> 0;
      else fl = (bl ^ (cl | ~dl)) >>> 0;
      // Right line applies the functions in a different order
      // (rounds run "in reverse"): x^(y|~z), (x&z)|(y&~z), (x|~y)^z,
      // (x&y)|(~x&z), then plain xor — likewise over (b, c, d).
      let fr;
      if (r === 0) fr = (br ^ (cr | ~dr)) >>> 0;
      else if (r === 1) fr = ((br & dr) | (cr & ~dr)) >>> 0;
      else if (r === 2) fr = ((br | ~cr) ^ dr) >>> 0;
      else if (r === 3) fr = ((br & cr) | (~br & dr)) >>> 0;
      else fr = (br ^ cr ^ dr) >>> 0;
      const tl = add(rol(add(al, fl, X[R1[j]], KL[r]), RH1[j]), el);
      al = el; el = dl; dl = rol(cl, 10); cl = bl; bl = tl;
      const tr = add(rol(add(ar, fr, X[R2[j]], KR[r]), RH2[j]), er);
      ar = er; er = dr; dr = rol(cr, 10); cr = br; br = tr;
    }
    // Combine: note the cross-wiring between the two lines' outputs.
    // h0 takes the updated right-line d (dr + h1 + cl), not h0 + al + ar.
    const t = add(dr, h1, cl);
    const nh0 = t;
    const nh1 = add(h2, dl, er);
    const nh2 = add(h3, el, ar);
    const nh3 = add(h4, al, br);
    const nh4 = add(h0, bl, cr);
    h0 = nh0; h1 = nh1; h2 = nh2; h3 = nh3; h4 = nh4;
  }

  const out = new Uint8Array(20);
  for (const [i, h] of [h0, h1, h2, h3, h4].entries()) {
    out[i * 4] = h & 0xff;
    out[i * 4 + 1] = (h >>> 8) & 0xff;
    out[i * 4 + 2] = (h >>> 16) & 0xff;
    out[i * 4 + 3] = (h >>> 24) & 0xff;
  }
  return out;
}

// -- Base58 (Bitcoin alphabet) ---------------------------------------------

const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58Encode(bytes) {
  let zeros = 0;
  while (zeros < bytes.length && bytes[zeros] === 0) zeros++;
  const digits = [0];
  for (let i = zeros; i < bytes.length; i++) {
    let carry = bytes[i];
    for (let j = 0; j < digits.length; j++) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = Math.floor(carry / 58);
    }
  }
  let encoded = "";
  for (let i = digits.length - 1; i >= 0; i--) {
    if (encoded === "" && digits[i] === 0) continue; // drop leading "1"s here…
    encoded += BASE58_ALPHABET[digits[i]];
  }
  return "1".repeat(zeros) + encoded; // …they are re-added from zero bytes
}

// -- wallet crypto (mirrors src/wallet/wallet.rs) --------------------------

// Address derivation: SHA256(x||y) -> RIPEMD160 -> 0x00 prefix ->
// double-SHA256 checksum (first 4 bytes) -> Base58. `x`/`y` are the 32-byte
// halves of the uncompressed P-256 public key.
async function deriveAddress(x, y) {
  const xy = new Uint8Array(64);
  xy.set(x, 0);
  xy.set(y, 32);
  const hashedKey = await sha256(xy);
  const ripe = ripemd160(hashedKey);
  const versioned = new Uint8Array(21);
  versioned[0] = 0x00;
  versioned.set(ripe, 1);
  const check1 = await sha256(versioned);
  const check2 = await sha256(check1);
  const full = new Uint8Array(25);
  full.set(versioned, 0);
  full.set(check2.subarray(0, 4), 21);
  return base58Encode(full);
}

// Signature bytes -> raw 64-byte r||s. Browsers emit DER-encoded ECDSA
// signatures; node-style producers may already hand over raw r||s
// (exactly 64 bytes), which passes through untouched.
function sigToRaw(sig) {
  if (sig.length === 64) return sig;
  return derToRawSig(sig);
}

function derToRawSig(der) {
  let o = 0;
  if (der[o++] !== 0x30) throw new Error("bad DER sequence");
  let seqLen = der[o++];
  if (seqLen & 0x80) o += seqLen & 0x7f; // long-form length: skip length bytes
  const takeInt = () => {
    if (der[o++] !== 0x02) throw new Error("bad DER integer");
    const len = der[o++];
    const v = der.subarray(o, o + len);
    o += len;
    let start = 0;
    while (start < v.length - 1 && v[start] === 0) start++;
    let t = v.subarray(start);
    if (t.length > 32) t = t.subarray(t.length - 32);
    const out = new Uint8Array(32);
    out.set(t, 32 - t.length);
    return out;
  };
  const raw = new Uint8Array(64);
  raw.set(takeInt(), 0);
  raw.set(takeInt(), 32);
  return raw;
}

// Canonical sign payload. Must stay byte-identical to the Rust side's
// serde_json of the unsigned wallet transaction:
// {"sender":…,"recipeint":…,"amount":N,"public_key":…,"signature":""}
// (field order and the historical "recipeint" spelling included).
function canonicalPayload(sender, recipient, amountNum, publicKeyHex) {
  const json = JSON.stringify({
    sender: sender,
    recipeint: recipient,
    amount: amountNum,
    public_key: publicKeyHex,
    signature: "",
  });
  return new TextEncoder().encode(json);
}

// Node harness: `require("wallet.js")` with a `$` stub (see bottom guard).
if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    hexEncode,
    hexDecode,
    sha256,
    ripemd160,
    base58Encode,
    deriveAddress,
    sigToRaw,
    derToRawSig,
    canonicalPayload,
  };
}

// -- page behaviour --------------------------------------------------------

$(function () {
  // Keypair lives here only: CryptoKey objects, never serialized to DOM.
  let wallet = null;

  function show_status(msg, is_error) {
    const el = $("#status");
    el.text(msg);
    el.addClass("show");
    el.removeClass("ok err");
    el.addClass(is_error ? "err" : "ok");
  }
  function server_error(xhr) {
    // Actix returns JSON-encoded strings, e.g. "\"insufficient ...\"".
    try {
      const body = JSON.parse(xhr.responseText);
      return typeof body === "string" ? body : JSON.stringify(body);
    } catch (e) {
      return xhr.responseText || xhr.statusText || "request failed";
    }
  }

  // 1. Generate a fresh wallet in the browser on every page load.
  async function newLocalWallet() {
    try {
      const keypair = await crypto.subtle.generateKey(
        { name: "ECDSA", namedCurve: "P-256" },
        false, // non-extractable: the private key cannot leave WebCrypto
        ["sign", "verify"],
      );
      const raw = new Uint8Array(await crypto.subtle.exportKey("raw", keypair.publicKey));
      const x = raw.subarray(1, 33);
      const y = raw.subarray(33, 65);
      const publicKeyHex = hexEncode(x) + hexEncode(y);
      const address = await deriveAddress(x, y);
      wallet = { privateKey: keypair.privateKey, publicKeyHex, address };
      $("#public_key").val(publicKeyHex);
      $("#blockchain_address").val(address);
      reload_amount();
    } catch (e) {
      show_status("Wallet needs WebCrypto (use localhost or HTTPS): " + e, true);
    }
  }
  newLocalWallet();

  $("#new_wallet").click(function () {
    newLocalWallet();
    show_status("New wallet generated locally. Fund it via the faucet.", false);
  });

  $("#send_money").click(async function () {
    if (!wallet) {
      show_status("Wallet not loaded yet", true);
      return;
    }
    const recipient = $("#recipient_address").val().trim();
    const amount = $("#send_amount").val().trim();
    if (!recipient) {
      show_status("Recipient address must not be empty", true);
      return;
    }
    if (!Number.isSafeInteger(Number(amount)) || Number(amount) <= 0) {
      show_status("Amount must be a positive integer", true);
      return;
    }
    if (!confirm("Are you ready to send " + amount + " to " + recipient + "?")) {
      show_status("Transaction cancelled", true);
      return;
    }

    try {
      // Sign locally; the server only ever sees the signature.
      const msg = canonicalPayload(wallet.address, recipient, Number(amount), wallet.publicKeyHex);
      const der = new Uint8Array(
        await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, wallet.privateKey, msg),
      );
      const transaction = {
        blockchain_address: wallet.address,
        recipient_address: recipient,
        amount: amount,
        public_key: wallet.publicKeyHex,
        signature: hexEncode(sigToRaw(der)),
      };
      $.ajax({
        url: "/transaction",
        method: "POST",
        contentType: "application/json",
        data: JSON.stringify(transaction),
        success: function (response) {
          show_status(
            "Queued: " + response.amount + " -> " + response.recipient + ". Press Mine to confirm.",
            false,
          );
        },
        error: function (xhr) {
          show_status("Transaction failed (" + xhr.status + "): " + server_error(xhr), true);
        },
      });
    } catch (e) {
      show_status("Signing failed: " + e, true);
    }
  });

  // 2. Faucet: new wallets start at 0, so mint test coins first.
  $("#get_faucet").click(function () {
    const address = $("#blockchain_address").val().trim();
    const amount = $("#faucet_amount").val().trim() || "100";
    if (!address) {
      show_status("Wallet not loaded yet", true);
      return;
    }
    $.ajax({
      url: "/faucet",
      method: "POST",
      contentType: "application/json",
      data: JSON.stringify({ recipient_address: address, amount: amount }),
      success: function (response) {
        show_status("Faucet queued " + response.amount + ". Press Mine to confirm.", false);
      },
      error: function (xhr) {
        show_status("Faucet failed (" + xhr.status + "): " + server_error(xhr), true);
      },
    });
  });

  // 3. Mining confirms queued (faucet + transfer) transactions.
  $("#mine").click(function () {
    $.ajax({
      url: "/mining",
      method: "GET",
      success: function (response) {
        show_status("Mined a block. Balance updated below.", false);
        reload_amount();
      },
      error: function (xhr) {
        show_status("Mining failed (" + xhr.status + "): " + server_error(xhr), true);
      },
    });
  });

  function reload_amount() {
    const address = $("#blockchain_address").val();
    if (!address || !address.trim()) {
      return;
    }
    const url = "/amount/" + encodeURIComponent(address.trim());
    $.ajax({
      url: url,
      type: "GET",
      success: function (response) {
        let amount = response["amount"];
        $("#input_amount").text(amount);
      },
      error: function (xhr) {
        show_status("Balance lookup failed: " + server_error(xhr), true);
      },
    });
  }

  $("#refresh_wallet").click(function () {
    reload_amount();
  });

  setInterval(reload_amount, 3000);
});
