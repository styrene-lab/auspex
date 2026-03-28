# Auspex in the Black Meridian Stack

## Purpose

Describe Auspex's role within the broader Black Meridian stack.

The canonical stack doctrine and product ontology live at the Black Meridian layer, not inside the Auspex repo. This document only explains how Auspex fits into that higher-level structure.

## Stack role

Within the Black Meridian stack:
- LLMs are how we think
- Omegon is how we reason and act
- Styrene is who we are and how we communicate
- Auspex is how we see and steer

Auspex is therefore the:
- interface layer
- operator shell
- local host application
- remote client experience

## What Auspex should own

Auspex should own:
- operator experience
- local session hosting
- subsystem supervision for bundled Omegon and Styrene components
- desktop and phone UI projections
- visibility, inspection, and steering

## What Auspex should not own

Auspex should not try to become:
- the reasoning engine itself (Omegon)
- the identity and communications substrate itself (Styrene)
- the cognition backend itself (LLMs)

## Why this matters

This keeps Auspex from becoming a compensating wrapper around backend accidents or a monolith that absorbs the whole stack.

It should remain a first-party product shell with clear subsystem boundaries.

## Related canonical notes

Canonical doctrine is maintained outside the Auspex repo:
- `obsidian/Black Meridian/Black Meridian Stack Doctrine.md`
- `obsidian/Black Meridian/Black Meridian Product Ontology.md`
