# Fldigi: Sending and Receiving Commands

Fldigi is controlled remotely over **XML-RPC**, an RPC protocol built on plain
HTTP POST + XML. There's no separate "sparrow protocol" to design here — the
`crates/fldigi` client just speaks XML-RPC to fldigi's built-in server. This
is the same interface flrig, logger32, and Xlog already use.

Verified against the actual `fldigi 4.2.13` binary installed on this machine
(`fldigi --xmlrpc-list`), cross-checked against upstream's
`fldigi_doxygen/user_src_docs/xmlrpc-control.txt`.

## Server basics

- Transport: XML-RPC over HTTP, endpoint `/RPC2`.
- Default address: `127.0.0.1:7362` — full URL: `http://127.0.0.1:7362/RPC2`.
- Enabled by default in this build (no compile flag needed; confirmed with
  `--xmlrpc-list`, which just talks to the compiled-in dispatcher and works
  without a running instance).

Relevant fldigi command-line flags:

```
--xmlrpc-server-address HOSTNAME   Default 127.0.0.1
--xmlrpc-server-port PORT          Default 7362
--xmlrpc-allow REGEX               Allow only methods matching REGEX
--xmlrpc-deny REGEX                Allow only methods NOT matching REGEX
--xmlrpc-list                      List all available methods and exit
```

`--xmlrpc-deny` is worth knowing about even if sparrow never sets it: it's
how a user could lock out transmit-capable methods, e.g.
`--xmlrpc-deny 'main\.(tx|tune|run_macro)'`.

## Rust side: the `xmlrpc` crate

No FFI, no linking fldigi's bundled `libflxmlrpc` — that's fldigi's *own*
internal C++ implementation and irrelevant to a Rust client. Plain HTTP.
Use the [`xmlrpc`](https://crates.io/crates/xmlrpc) crate (client/server XML-RPC
over `xml-rs` + `reqwest`):

```toml
# crates/fldigi/Cargo.toml
[dependencies]
xmlrpc = "0.15"
```

```rust
use xmlrpc::{Request, Value};

const FLDIGI_URL: &str = "http://127.0.0.1:7362/RPC2";

// Sending a command with arguments:
let old_freq = Request::new("main.set_frequency")
    .arg(14074000.0_f64)
    .call_url(FLDIGI_URL)?;

// Sending a command with no arguments:
Request::new("main.tx").call_url(FLDIGI_URL)?;

// Receiving data back:
let rx_text: Value = Request::new("text.get_rx_length").call_url(FLDIGI_URL)?;
```

`call_url` returns `Result<Value, xmlrpc::Error>`. `Value` is an enum
(`Value::String`, `Value::Int`, `Value::Double`, `Value::Bool`,
`Value::Array`, `Value::Struct`, `Value::Nil`, ...) matching the signature
column below.

### Signature symbols

| Symbol | Type |
|---|---|
| `n` | nil (no value / no argument) |
| `b` | boolean |
| `i` | integer |
| `d` | double |
| `s` | string |
| `6` | base64 byte string |
| `A` | array |
| `S` | struct |

A signature is written `return:args`, e.g. `d:d` = takes a double, returns a
double.

## Sending commands

### Transmit control (`main.*`)

| Method | Sig | Description |
|---|---|---|
| `main.tx` | n:n | Start transmitting |
| `main.tune` | n:n | Start tuning (carrier, no data) |
| `main.rx` | n:n | Switch to receive |
| `main.abort` | n:n | Abort a transmit or tune in progress |
| `main.rx_only` | n:n | Disable Tx entirely |
| `main.rx_tx` | n:n | Restore normal Rx/Tx switching |
| `main.run_macro` | n:i | Run macro by ID |

### Sending text/data to transmit

| Method | Sig | Description |
|---|---|---|
| `text.add_tx` | n:s | Append a string to the TX text widget |
| `text.add_tx_bytes` | n:6 | Append a base64 byte string to TX |
| `text.add_tx_queu` | n:s | Append a string to the TX transmit queue |
| `text.clear_tx` | n:n | Clear the TX text widget |

Typical pattern: `text.add_tx` the outgoing text, then `main.tx` to key up
and send it.

### Rig/frequency/mode control (`rig.*`)

fldigi can act as an XML-RPC-controlled "virtual rig" for a controller
program, or just report/set the frequency of the physical rig it's already
talking to via hamlib — same method set either way:

| Method | Sig | Description |
|---|---|---|
| `rig.set_frequency` | d:d | Set RF carrier frequency, returns old value |
| `rig.get_frequency` | d:n | Get RF carrier frequency |
| `rig.set_mode` / `rig.get_mode` | n:s / s:n | Set/get transceiver mode |
| `rig.set_modes` / `rig.get_modes` | n:A / A:n | Set/get the list of available modes |
| `rig.set_bandwidth` / `rig.get_bandwidth` | n:s / s:n | Set/get bandwidth |
| `rig.take_control` / `rig.release_control` | n:n / n:n | Switch rig control to/from XML-RPC |
| `rig.enable_qsy` | n:i | Enable/disable QSY (frequency-follow) for XML-RPC rig control |

`main.set_frequency`/`main.get_frequency` still exist but are deprecated in
favor of the `rig.*` equivalents.

### Modem/mode selection (`modem.*`)

| Method | Sig | Description |
|---|---|---|
| `modem.set_by_name` | s:s | Select modem by name (e.g. `"BPSK31"`), returns old name |
| `modem.set_by_id` | i:i | Select modem by numeric ID, returns old ID |
| `modem.get_name` / `modem.get_names` | s:n / A:n | Current modem name / all modem names |
| `modem.set_carrier` | i:i | Set audio carrier frequency (Hz) |
| `modem.set_bandwidth` | i:i | Set modem bandwidth |

## Receiving data

XML-RPC is request/response, not a push/subscribe protocol — there's no
server-initiated notification when new decoded text arrives. The client has
to poll.

| Method | Sig | Description |
|---|---|---|
| `text.get_rx_length` | i:n | Number of characters currently in the RX widget |
| `text.get_rx` | 6:ii | Returns a range `(start, length)` of RX text as base64 bytes |
| `text.clear_rx` | n:n | Clear the RX text widget |
| `rx.get_data` | 6:n | All RX data received since the last call to this method |
| `tx.get_data` | 6:n | All TX data transmitted since the last call |
| `rxtx.get_data` | 6:n | Combined RX+TX data since the last call |

`rx.get_data`/`tx.get_data`/`rxtx.get_data` are the simplest polling
primitive for a "watch the decoded stream" feature: call on an interval
(e.g. every 250ms–1s) and append whatever comes back — each call only
returns what's new since the previous call, so nothing needs to be tracked
client-side beyond the connection itself.

### Status/state queries

| Method | Sig | Description |
|---|---|---|
| `main.get_trx_status` | s:n | `"tx"` / `"rx"` / `"tune"` |
| `main.get_trx_state` | s:n | T/R state |
| `modem.get_quality` | d:n | Signal quality, 0–100 |
| `fldigi.name_version` | s:n | Program name + version, useful as a liveness/connectivity check |

## Full method reference (this installed version, 4.2.13)

Grouped by namespace. `[DEPRECATED]` entries are kept for compatibility;
prefer their replacement.

<details>
<summary>fldigi.* — program-level</summary>

| Method | Sig | Description |
|---|---|---|
| `fldigi.list` | A:n | Returns the list of methods |
| `fldigi.name` | s:n | Program name |
| `fldigi.name_version` | s:n | Program name + version |
| `fldigi.version` | s:n | Program version as a string |
| `fldigi.version_struct` | S:n | Program version as a struct |
| `fldigi.config_dir` | s:n | Configuration directory path |
| `fldigi.terminate` | n:i | Terminate fldigi (bitmask: 0=options,1=log,2=macros) |

</details>

<details>
<summary>modem.* — modem/mode selection</summary>

| Method | Sig | Description |
|---|---|---|
| `modem.get_mode` | s:n | ADIF mode of current modem |
| `modem.get_submode` | s:n | ADIF submode of current modem |
| `modem.get_name` / `get_names` / `get_io_names` | s:n / A:n / A:n | Modem name(s) |
| `modem.get_id` / `get_max_id` | i:n / i:n | Modem ID / max ID |
| `modem.set_by_name` / `set_by_id` | s:s / i:i | Select modem |
| `modem.set_carrier` / `get_carrier` / `inc_carrier` | i:i / i:n / i:i | Carrier frequency |
| `modem.set_afc_search_range` / `get_afc_search_range` / `inc_afc_search_range` | i:i / i:n / i:i | AFC search range |
| `modem.set_bandwidth` / `get_bandwidth` / `inc_bandwidth` | i:i / i:n / i:i | Bandwidth |
| `modem.get_quality` | d:n | Signal quality [0:100] |
| `modem.search_up` / `search_down` | n:n / n:n | Search for signal |
| `modem.olivia.set_bandwidth` / `get_bandwidth` | n:i / i:n | Olivia-specific bandwidth |
| `modem.olivia.set_tones` / `get_tones` | n:i / i:n | Olivia-specific tones |

</details>

<details>
<summary>main.* — transceiver/session control</summary>

| Method | Sig | Description |
|---|---|---|
| `main.tx` / `tune` / `rx` / `abort` | n:n each | Transmit / tune / receive / abort |
| `main.rx_only` / `rx_tx` | n:n / n:n | Disable Tx / restore normal switching |
| `main.get_status1` / `get_status2` | s:n / s:n | Status field contents |
| `main.get_wf_sideband` / `set_wf_sideband` | s:n / n:s | Waterfall sideband |
| `main.set_frequency` / `inc_frequency` | d:d / d:d | Frequency (prefer `rig.*`) |
| `main.get_frequency` | d:n | `[DEPRECATED; use rig.get_frequency]` |
| `main.get_afc` / `set_afc` / `toggle_afc` | b:n / b:b / b:n | AFC state |
| `main.get_squelch` / `set_squelch` / `toggle_squelch` | b:n / b:b / b:n | Squelch state |
| `main.get_squelch_level` / `set_squelch_level` / `inc_squelch_level` | d:n / d:d / d:d | Squelch level |
| `main.get_reverse` / `set_reverse` / `toggle_reverse` | b:n / b:b / b:n | Reverse sideband |
| `main.get_lock` / `set_lock` / `toggle_lock` | b:n / b:b / b:n | Transmit lock |
| `main.get_txid` / `set_txid` / `toggle_txid` | b:n / b:b / b:n | TxRSID state |
| `main.get_rsid` / `set_rsid` / `toggle_rsid` | b:n / b:b / b:n | RSID state |
| `main.get_trx_status` / `get_trx_state` | s:n / s:n | Transmit/tune/receive status / T-R state |
| `main.get_tx_timing` / `get_char_rates` / `get_char_timing` | n:s / s:n / n:i | Timing info |
| `main.run_macro` / `get_max_macro_id` | n:i / i:n | Macros |
| `main.flmsg_online` / `flmsg_available` / `flmsg_transfer` / `flmsg_squelch` | n:n / n:n / n:n / b:n | flmsg bridge (superseded by `flmsg.*`) |
| `main.get_sideband`, `set_sideband`, `rsid`, `set_rig_*`, `get_rig_*` | — | `[DEPRECATED]` — use `main.get_wf_sideband`/`rig.*`/`main.{get,set,toggle}_rsid` |

</details>

<details>
<summary>rig.* — rig control (frequency/mode/bandwidth)</summary>

| Method | Sig | Description |
|---|---|---|
| `rig.set_name` / `get_name` | n:s / s:n | Rig name for XML-RPC rig |
| `rig.set_frequency` / `get_frequency` | d:d / d:n | Frequency |
| `rig.set_mode` / `get_mode` / `set_modes` / `get_modes` | n:s / s:n / n:A / A:n | Mode |
| `rig.set_bandwidth` / `get_bandwidth` / `set_bandwidths` / `get_bandwidths` | n:s / s:n / n:A / A:n | Bandwidth |
| `rig.set_smeter` / `set_pwrmeter` | n:i / n:i | Meter values |
| `rig.get_notch` / `set_notch` | s:n / n:i | Notch filter |
| `rig.enable_qsy` | n:i | Enable/disable QSY for XML-RPC rig control |
| `rig.take_control` / `release_control` | n:n / n:n | Switch rig control to/from XML-RPC |

</details>

<details>
<summary>log.* / logbook.* — logging fields</summary>

Get/set pairs for the log entry fields: `call`, `name`, `qth`, `locator`,
`rst_in`, `rst_out`, `serial_number`, `serial_number_sent`, `exchange`,
`frequency`, `band`, `time_on`/`time_off`, `date_on`/`date_off`, `state`,
`province`, `country`, `notes`, `az`. Plus:

| Method | Sig | Description |
|---|---|---|
| `log.clear` | n:n | Clear the log fields |
| `log.set_contest_counter` | n:s | Set starting contest number |
| `log.get_sideband` | s:n | `[DEPRECATED; use main.get_wf_sideband]` |
| `logbook.last_record` | s:n | ADIF record of the last logged QSO |
| `logbook.all_records` | s:n | Entire ADIF logbook currently open |

</details>

<details>
<summary>text.* / rx.* / tx.* / rxtx.* — RX/TX text streams</summary>

| Method | Sig | Description |
|---|---|---|
| `text.get_rx_length` | i:n | Character count in RX widget |
| `text.get_rx` | 6:ii | Range `(start, length)` from RX widget, base64 |
| `text.clear_rx` / `clear_tx` | n:n / n:n | Clear widgets |
| `text.add_tx` / `add_tx_bytes` / `add_tx_queu` | n:s / n:6 / n:s | Append to TX |
| `rx.get_data` / `tx.get_data` / `rxtx.get_data` | 6:n each | New data since last call |

</details>

<details>
<summary>io.* / flmsg.* / spot.* — I/O mode, flmsg bridge, PSK Reporter</summary>

| Method | Sig | Description |
|---|---|---|
| `io.in_use` | s:n | Current I/O port (ARQ/KISS) |
| `io.enable_kiss` / `enable_arq` | n:n / n:n | Switch I/O mode |
| `flmsg.online` / `available` / `transfer` / `squelch` | n:n / n:n / n:n / b:n | flmsg bridge state |
| `flmsg.get_data` | 6:n | RX data for flmsg since last query |
| `spot.get_auto` / `set_auto` / `toggle_auto` | b:n / b:b / b:n | PSK Reporter autospotter |
| `spot.pskrep.get_count` | i:n | Callsigns spotted this session |

</details>

<details>
<summary>wefax.* / navtex.* — fax and Navtex modes</summary>

| Method | Sig | Description |
|---|---|---|
| `wefax.state_string` | s:n | Engine state (tx and rx) |
| `wefax.skip_apt` / `skip_phasing` | s:n / s:n | Skip reception phases |
| `wefax.set_tx_abort_flag` / `end_reception` | s:n / s:n | Cancel transmit / end receive |
| `wefax.start_manual_reception` | s:n | Manual-mode fax reception |
| `wefax.set_adif_log` | s:b | Log fax images to ADIF |
| `wefax.set_max_lines` | s:i | Max lines for received image |
| `wefax.get_received_file` | s:i | Wait for next received file (timeout in seconds) |
| `wefax.send_file` | s:si | Send a fax image file |
| `navtex.get_message` | s:i | Wait for next Navtex/SitorB message (timeout in seconds) |
| `navtex.send_message` | s:s | Send a Navtex/SitorB message |

</details>

## Open question for the `crates/fldigi` design

fldigi's own polling model (`rx.get_data` etc.) means sparrow will need its
own polling loop/task on the Rust side to turn this into something
event-like for the UI (e.g. a channel that the Tauri frontend subscribes
to). That's an implementation decision for whoever builds `crates/fldigi`,
not something this reference doc settles.
