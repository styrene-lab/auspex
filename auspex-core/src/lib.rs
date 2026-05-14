//! Auspex Core — logic, types, and state machines for Omegon fleet management.
//!
//! This crate contains everything except the Dioxus UI shell:
//! - Control plane types and protocol parsing
//! - Instance registry, health probing, and lifecycle state machines
//! - Session models and event processing
//! - WebSocket and IPC transport
//! - Bootstrap and discovery

// Foundation (no internal deps)
pub mod cop_surface;
pub mod omegon_control;
pub mod runtime_types;
pub mod secret_grants;
pub mod secret_orchestration;
pub mod session_model;

// Data layer
#[cfg(not(target_arch = "wasm32"))]
pub mod config;
pub mod fixtures;
pub mod session_event;

// Infrastructure
pub mod audit_timeline;
pub mod command_transport;
#[cfg(not(target_arch = "wasm32"))]
pub mod container_discovery;
pub mod event_stream;
pub mod instance_registry;
#[cfg(not(target_arch = "wasm32"))]
pub mod ipc_client;
#[cfg(not(target_arch = "wasm32"))]
pub mod tls_config;

// State machines
pub mod cop_feature;
pub mod instance_session;
pub mod remote_session;
pub mod state_engine;
pub mod telemetry;

// Orchestration
pub mod bootstrap;
pub mod controller;
