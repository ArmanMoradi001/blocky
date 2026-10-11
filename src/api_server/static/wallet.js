$(function () {
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

  // 1. Create a fresh wallet for this page load.
  $.ajax({
    url: "/get_wallet",
    method: "GET",
    success: function (response) {
      console.log(response);
      $("#public_key").val(response["public_key"]);
      $("#private_key").val(response["private_key"]);
      $("#blockchain_address").val(response["blockchain_address"]);
      reload_amount();
    },
    error: function (xhr) {
      console.log(xhr);
      show_status("Failed to create wallet: " + server_error(xhr), true);
    },
  });

  $("#send_money").click(function () {
    const recipient = $("#recipient_address").val().trim();
    const amount = $("#send_amount").val().trim();
    if (!recipient) {
      show_status("Recipient address must not be empty", true);
      return;
    }
    if (!amount) {
      show_status("Amount must not be empty", true);
      return;
    }
    if (!confirm("Are you ready to send " + amount + " to " + recipient + "?")) {
      show_status("Transaction cancelled", true);
      return;
    }

    const transaction = {
      private_key: $("#private_key").val(),
      blockchain_address: $("#blockchain_address").val(),
      public_key: $("#public_key").val(),
      recipient_address: recipient,
      amount: amount,
    };

    $.ajax({
      url: "/transaction",
      method: "POST",
      contentType: "application/json",
      data: JSON.stringify(transaction),
      success: function (response) {
        console.log(response);
        show_status(
          "Queued: " + response.amount + " -> " + response.recipient + ". Press Mine to confirm.",
          false,
        );
      },
      error: function (xhr) {
        console.error(xhr);
        show_status("Transaction failed (" + xhr.status + "): " + server_error(xhr), true);
      },
    });
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
        console.log(response);
        show_status("Faucet queued " + response.amount + ". Press Mine to confirm.", false);
      },
      error: function (xhr) {
        console.error(xhr);
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
        console.log(response);
        show_status("Mined a block. Balance updated below.", false);
        reload_amount();
      },
      error: function (xhr) {
        console.error(xhr);
        show_status("Mining failed (" + xhr.status + "): " + server_error(xhr), true);
      },
    });
  });

  function reload_amount() {
    const address = $("#blockchain_address").val();
    if (!address || !address.trim()) {
      return;
    }
    const url = "/amount/" + address.trim();
    $.ajax({
      url: url,
      type: "GET",
      success: function (response) {
        let amount = response["amount"];
        $("#input_amount").text(amount);
        console.log(amount);
      },
      error: function (xhr) {
        console.error(xhr);
      },
    });
  }

  $("#refresh_wallet").click(function () {
    reload_amount();
  });

  setInterval(reload_amount, 3000);
});
