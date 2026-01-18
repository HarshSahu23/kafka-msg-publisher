# Tauri + Angular

This template should help get you started developing with Tauri and Angular.

## Tech Stack

* Angular (TypeScript) : Frontend framework used to build the User Interface (inputs, buttons, logs).
* Rust : Backend language used for high-performance networking, Kafka TCP connections, and OS interactions.
* Tauri : Application container that bridges the Frontend and Backend, managing the application window and security.
* Angular CLI : Project management toolchain that orchestrates the build process (using Vite/Esbuild internally).
* rs-kafka : Pure Rust library for connecting to Kafka brokers without external C/C++ dependencies.
* tokio : Asynchronous runtime engine required to execute non-blocking network tasks in Rust.
* serde & serde_json : Rust libraries for serializing and deserializing data (translating JSON from Frontend to Rust structs).
* @tauri-apps/api : JavaScript library used in Angular to send commands (`invoke`) to the Rust backend.
* @tauri-apps/cli : Command-line utility for initializing, developing (`tauri dev`), and building (`tauri build`) the application.
* WebView2 (Windows) / WebKit (Linux/Mac) : The native OS rendering engine that displays the Angular application.


