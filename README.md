# Tube

```
NOTE: Tube is a side-project of mine that I spend varying amounts of time on. My original goal for this project was to get more hands-on experience
with how Rust's concurrency abstractions work and perform. As such, I've spent most of my time exploring how the design fits within the constraints
of Rust and very little (AKA zero) time on polishing some of the API details, writing docs, or attempting to use Tubez in a real-world use case -- but
each of those things are on my mental roadmap.

I do not recommend trying to do anything with this library as it is still in an exploratory state.
```

Tubez is an abstraction over http2/3 (and, eventually, websocket and webtransport) for establishing long-lived, uni- and bi-directional streams of binary
data (called a "Tube") between a client and a server with an extremely simple API. It supports both client- and server-initiated Tubes, Tube-lifecycle
management, and uses a custom framing protocol with future intention to support intelligent routing and load-balancing of Tubes with minimal compromise to
data privacy.
