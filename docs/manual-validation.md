# Physical-device release checks

These optional real-device/hosted checks are unverified. They are not required
to publish the local clone-and-run repository. Automated Chromium contexts, loopback TURN and virtual
partitions do not substitute for devices on different networks. Record the tested
commit, date, OS/browser versions, device, networks, TURN configuration and result.
Never attach invitation tokens, document content, full SDP or ICE candidates to
public logs.

1. Deploy the stack in [deployment.md](deployment.md), verify HTTPS, WSS and the
   health endpoint, and open a new room on a laptop and a phone using different
   networks. Confirm both can type and see the other's cursor/name.
2. Disconnect the phone's actual network. Keep editing on both devices. Restore
   connectivity and check both histories are retained and the full text agrees.
   Repeat after changing Wi-Fi/mobile networks and after a longer offline period.
3. Open `?relay#ROOM_ID` on both devices and confirm **Connected via relay** from
   selected ICE statistics. Repeat editing and reconnect. Test any enterprise
   network requiring TURN over TLS separately; the supplied stack uses port 3478.
4. Stop signaling after establishing a connection and continue editing. New joins
   and ICE repair can fail until signaling returns. Keep TURN running during this
   check, and distinguish relay dependency from signaling dependency.
5. Background and suspend one browser, edit elsewhere, then resume. A suspicion
   is provisional; reconnect and retained-history repair must converge. Repeat
   with mobile screen lock and desktop tab throttling.
6. Use a real OS Japanese/Chinese/Korean IME while another peer types. Test emoji,
   combining marks, multiline replacement, selection, paste and cursor positions.
   Automated Chromium composition covers a narrower path.
7. Wait for **Saved on this device**, close the browser, reopen and reload offline
   after the app shell has been cached. Verify work remains. Force a storage
   denial/quota error and ensure failure is visible; unsaved work is not durable.
8. Export plain text and a binary ConV backup. Restore the backup in a fresh
   browser profile and merge with a surviving peer. Plain-text import should
   create a new room. Clearing every replica must leave no cloud recovery claim.
9. Try a ninth participant and verify the capacity error. Check malformed invites,
   unavailable signaling, expired credentials and unavailable peers give useful
   connection states without losing the local document.

Container builds and the public-host checklist must be run on a Docker-enabled
host. The current WSL environment has Docker Desktop integration disabled.
