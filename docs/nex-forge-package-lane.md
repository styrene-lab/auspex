---
id: nex-forge-package-lane
title: "Nex forge and Styrene distributed package lane"
status: design target
parent: auspex-runtime-backends
tags:
  - nex
  - deployment
  - packages
  - omegon
dependencies:
  - auspex-runtime-backends
  - auspex-primary-coordinator
related:
  - auspex-runtime-backends
  - auspex-primary-coordinator
  - operator-security-tiers
---

# Nex forge and Styrene distributed package lane

## Assessment

Nex already has several primitives that are useful for Styrene-wide "create and deploy" packaging, but its current `forge` command is mostly a host bootstrap flow, not the right core primitive for deploying individual agents.

The relevant Nex surfaces are:

- `profile.toml` with `extends` and `compose` for layered package/environment definitions.
- `nex profile sign` / `verify` for StyreneIdentity-backed profile authenticity.
- `nex build-image` for turning a profile into an OCI image through Nix `dockerTools`.
- `nex forge` for creating a NixOS installer bundle with profile/defaults/nex baked in.
- `nex polymerize` for consuming that bundle on a target host and installing NixOS.
- `nex identity` and `nex rbac sync` for operator identity, derived keys, and hub-roster projection.

The usable shape is therefore:

- Nex builds trusted environments and host bootstrap media.
- Omegon is the agent runtime inside those environments.
- Auspex chooses, instantiates, supervises, and reconciles the deployed agents.
- Signum or Armory should catalog signed package manifests and image references.

## Boundary

`nex forge` should remain the host and node bootstrap mechanism.

It is appropriate for:

- provisioning a worker machine
- baking a Styrene profile into a NixOS installer
- preparing a cluster node or isolated operator host
- carrying defaults, identity bootstrap material, and first-run install state

It is not the ideal primitive for:

- creating one logical Omegon worker
- scaling agent replicas
- updating a running fleet
- selecting model/tool posture
- managing runtime control-plane attachment

Those concerns belong to Auspex runtime backends and the Omegon agent lifecycle.

## Package Model

Styrene needs a distributed package concept that sits above Nex profiles but below Auspex runtime intent.

A package should describe:

- identity: package name, version, signer, source, provenance
- environment: Nex profile ref or resolved profile content
- image: OCI image name, digest, entrypoint, exposed control-plane ports
- agent: Omegon role, posture, mode, model defaults, thinking defaults
- capabilities: skills, armory packages, MCP endpoints, workflow bindings
- policy: required approvals, allowed backends, secret refs, network posture
- deploy: default backend profile, resources, namespace, replica/daemon semantics

Minimal package manifest sketch:

```toml
[package]
name = "styrene.agent.primary-driver"
version = "0.1.0"
source = "github:styrene-lab/agent-packages/primary-driver"

[nex]
profile = "styrene-lab/nex-profiles/omegon-agent"
verify = true

[image]
name = "ghcr.io/styrene-lab/omegon-agent-primary"
tag = "0.1.0"
entrypoint = "/bin/omegon"
cmd = ["serve", "--control-plane", "0.0.0.0:7842"]
ports = [7842]

[agent]
role = "primary-driver"
mode = "daemon"
posture = "orchestrator"
model = "anthropic:claude-sonnet-4-6"
thinking = "medium"

[capabilities]
skills = ["styrene.coordinator", "styrene.deployment"]
armory = ["omegon-flow", "sentry-tasking"]

[deploy]
backends = ["local-process", "oci-container", "kubernetes"]
default_backend = "kubernetes"
namespace = "omegon-agents"
cpu = "1"
memory = "2Gi"
```

This manifest should be signed independently of the resolved profile. Profile signing says "this environment definition is trusted." Package signing says "this deployable agent package is trusted."

## Create And Deploy Flow

The target workflow should be:

1. Author a Nex profile or compose an existing base profile.
2. Add an agent package manifest that references the Nex profile and declares Omegon runtime posture.
3. Build an OCI image from the profile and package runtime settings.
4. Sign the resolved package manifest with StyreneIdentity.
5. Publish the image and signed manifest to the Styrene package catalog.
6. Auspex discovers the package through Armory/Signum/catalog state.
7. Operator or primary coordinator stages a `DeploymentIntent`.
8. Auspex resolves the package into a backend-specific instantiate request.
9. Runtime backend launches the local process, OCI container, or Kubernetes `OmegonAgent`.
10. Instance registry records package identity, image digest, signer, control-plane URL, ACP URL, and lifecycle state.

## Nex Changes To Prefer

Do not overload current `nex forge` until its host-bootstrap semantics are split cleanly.

Better command shape:

- `nex package init`
- `nex package build`
- `nex package sign`
- `nex package publish`
- `nex package inspect`

`nex package build` can reuse `build-image` internally, but it should emit a first-class package bundle:

- `package.toml`
- resolved `profile.toml`
- signed manifest or detached signature
- OCI image digest or local tarball path
- SBOM/provenance when available
- optional Kubernetes values/manifest preview

`nex forge` can then remain explicitly host-oriented:

- `nex forge host`
- `nex forge node`
- `nex forge installer`

## Auspex Changes To Prefer

Auspex should consume packages as inputs to runtime intent, not generate ad hoc deployment specs in the UI.

Required additions:

- package catalog model: name, version, signer, image digest, capabilities, policy, deploy defaults
- package resolver: package + overrides -> canonical instantiate request
- deployment preview: show package diff, signer, image digest, requested secrets, backend, resources
- runtime registry fields: package id, version, signer, image digest, deployment intent id
- Graph view: show deployed package lineage per agent node
- Chat/direct-line view: show selected agent package and mutable config surfaces without exposing raw manifest noise

The primary coordinator should gain tools that operate at the package level:

- `package.list`
- `package.inspect`
- `package.plan_deploy`
- `deployment.stage`
- `deployment.apply`
- `deployment.rollback`

## First Slice

The smallest useful slice is not a full package registry.

1. Define `styrene-package.toml` for Omegon agent packages.
2. Teach Auspex deploy profiles to reference a package id and resolved image.
3. Add a Nex convention doc/example for building an Omegon agent image with `[container]`.
4. Add package metadata fields to the Auspex instance registry.
5. Add a local file-backed package catalog for development.
6. Map one package into the existing Kubernetes `OmegonAgent` path.

This gets "create and deploy" working without making `forge` responsible for fleet orchestration.

## Risks

- Nex profiles are currently machine/dev-environment oriented; agent posture and runtime policy need a separate schema.
- `nex build-image` produces local image artifacts and runtime suggestions, not registry push/deploy semantics.
- `forge` and `polymerize` are powerful host-install flows with larger blast radius than agent deployment needs.
- Package signing must bind source, resolved profile, image digest, agent manifest, and signer; signing only the profile is insufficient.
- Kubernetes deployment must use image digests or signed provenance, not mutable tags, for trustworthy reconciliation.

## Recommendation

Use Nex as the package/environment builder and host bootstrap tool. Do not make Nex the agent control plane.

For Styrene distributed packages, introduce a dedicated package manifest and package commands that reuse Nex profile resolution, signing, and image building. Auspex should then deploy those packages through its runtime backend abstraction, with Omegon remaining the concrete runtime inside each package.
