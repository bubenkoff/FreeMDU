# Miele W 471 Softtronic — software ID 457

Read-only dumps for adding ID 457 to FreeMDU. Taken on 2026-10-12 through the optical port
(XIAO ESP32-C6 + SFH 7250) with the read access key `0x43ea` only. Full access was not
unlocked and nothing was written to the machine.

Machine: Miele W 471 Softtronic, M.-Nr. 05 645 660, Dutch panel, 1600 rpm, manufactured
2002-02-28, 17 446 operating hours.

| File | Contents |
|---|---|
| `eeprom-0000-07ff.bin` | EEPROM, 2048 bytes, read twice with identical results (`ReadEeprom`, word address = byte offset / 2) |
| `mem-0400-1fff.bin` | Memory `0x0400–0x1fff`. Reads that touch `0x2000` or above are acknowledged but no data follows |
| `ram-0000-03ff-door-open.bin` | RAM snapshot, door open, dial on Finish |
| `ram-0000-03ff-door-closed-finish.bin` | door closed, dial on Finish |
| `ram-0000-03ff-cottons60-idle.bin` | door closed, Cottons 60 selected, not started |
| `ram-0000-03ff-cottons40-idle.bin` | door closed, Cottons 40 selected, not started |
| `ram-0000-03ff-cottons60-display-51.bin` | as above while the display showed "51" (remaining time) |
| `miniwas-cycle.ramlog` | a complete Miniwas (quick wash 40) run, empty drum: 103 records |
| `miniwas-cycle.log` | one text line per record of the run, with a few decoded bytes |
| `changes-spin-button-and-dial-sweep.log` | byte changes in `0x0040–0x00ff` while the spin button was stepped through all 8 settings and the dial through all 22 positions |
| `changes-option-buttons.log` | the same while pressing Inweken, Voorwas, Intensief, Extra water and Voorkeuze |
| `ramlog.py` | reader for the `.ramlog` format |

`.ramlog` format: records of 1032 bytes — an 8-byte little-endian Unix time in milliseconds,
then RAM `0x0000–0x03ff`. Records are about 20 s apart; one snapshot takes about 20 s to
read, so a record is not a single instant.

Change logs: `t=<seconds>` followed by `address:old>new` pairs; the polling cycle was about
3.7 s.

In the Miniwas run the program was started at about 1.3 min into the log and the machine
reported "finished" at 33.3 min.
