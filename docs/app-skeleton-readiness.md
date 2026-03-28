# App Skeleton Readiness Checklist

## Purpose

Define the minimum design and backend prerequisites before scaffolding a real Dioxus application in `auspex/`.

## Ready when

### Product/UI prerequisites
- [x] Vision and mode model are written
- [x] Screen-to-control-plane bindings exist
- [x] Compatibility handshake is defined
- [x] Error and empty states are defined

### Backend prerequisites
- [x] Control-plane contract direction exists
- [x] Omegon backend delta checklist exists
- [x] Type sketch for `ControlPlaneStateV1` exists
- [x] Release dependency policy exists
- [ ] Omegon actually exposes a compatible released control-plane contract

## Interpretation

Auspex is now ready for a code skeleton **only if** we accept that the first code may need mocked backend data or a stubbed protocol layer.

Auspex is **not** yet ready to harden a real backend integration against Omegon production behavior until the Omegon implementation slice lands.

## Recommended near-term tracks

### Track A — backend first
Implement the Omegon `ControlPlaneStateV1` slice first, then scaffold the real client.

### Track B — client shell with mocks
Build a Dioxus shell against mocked `ControlPlaneStateV1` payloads, then swap to the real backend after Omegon lands the contract.

## Recommendation

Track A is safer.
Track B is acceptable only if the shell is kept thin and avoids inventing client-side backend semantics.
