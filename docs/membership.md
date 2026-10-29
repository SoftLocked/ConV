# Membership

The host supplies monotonic milliseconds and a shuffled traversal of known peer
sessions. SWIM probes one target per second, waits 250 ms, asks up to three
helpers, then suspects after a further 250 ms. Suspicion expires after five
seconds. A peer refutes its own suspicion with a higher incarnation. Updates
compare incarnation first, then Alive < Suspect < Dead < Left. Restarts use new
session identities. Document data is retained regardless of membership status.

The prototype host polls membership at 100 ms intervals. Small-room membership
observations are repeated with periodic synchronization summaries to recover lost
updates. This uses the existing neighbor mesh; native seed discovery learns
reachable endpoints, while browser discovery will come from signaling. The
current full-table dissemination needs a bounded gossip budget before claiming
200-peer membership efficiency. Native peer address retention also needs churn
limits before long-running deployment.

Tests cover direct/indirect timeouts, helper acknowledgements, suspicion
refutation, stale updates and stale suspicion deadlines. Background browser
throttling and real WAN failure-detection behavior remain to be measured.

Dissemination now sends at most three new-observation forwards, and periodic
sync gossip carries at most three observations to three neighbors. Production
native/browser hosts supply fresh shuffled probe traversals. Native `/quit`
sends a voluntary leave. Observations are capped at 1,024 sessions; helper relay
tracking is capped at 256 entries with expiry. The full browser room remains
capped at eight peers. Long-lived native churn/address retirement still needs
further measurement.
