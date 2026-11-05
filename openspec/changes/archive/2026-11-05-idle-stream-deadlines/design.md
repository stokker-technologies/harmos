# Agreed Design

Owner agreement: 2026-11-05. Endpoint movement broadcasts progress without resetting the permanent retry fence. Runtime execution observes both directions and rearms idle time, including the final response wait. Unary admission, initialization, attempts and cleanup retain the total budget. Streaming attempts use the idle budget rather than an independent total attempt timer. Listener ingress retains bounded middleware and opening waits, then delegates streaming deadlines to operation execution. FOLLOW producers must emit heartbeats within their policy silence budget.

Dokime removes upload total grpc-timeout, derives upload/download silence bounds from operation policy and removes four duplicate literal deadlines. No stream size caps change. README reviewed: existing landing-page claims and examples are unaffected.
