//! Auspex Core — logic, types, and state machines for Omegon fleet management.
//!
//! This crate contains everything except the Dioxus UI shell:
//! - Control plane types and protocol parsing
//! - Instance registry, health probing, and lifecycle state machines
//! - Session models and event processing
//! - WebSocket and IPC transport
//! - Bootstrap and discovery

// Foundation (no internal deps)
pub mod runtime_types;
pub mod omegon_control;
pub mod cop_surface;
pub mod session_model;

// Data layer
pub mod fixtures;
pub mod session_event;
#[cfg(not(target_arch = "wasm32"))]
pub mod config;

// Infrastructure
pub mod instance_registry;
pub mod event_stream;
#[cfg(not(target_arch = "wasm32"))]
pub mod ipc_client;
pub mod command_transport;
pub mod audit_timeline;
#[cfg(not(target_arch = "wasm32"))]
pub mod container_discovery;

// State machines
pub mod state_engine;
pub mod telemetry;
pub mod remote_session;
pub mod cop_feature;
pub mod instance_session;

// Orchestration
pub mod controller;
pub mod bootstrap;
