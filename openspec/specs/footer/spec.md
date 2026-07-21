# Capability: Footer

## Purpose

Defines the TUI footer: a single terminal row split into static key-hints on the
left and a volatile status region (sync / PR-pending / warning) on the right.
Covers what each half shows and how the row behaves when space is tight.

## Requirements

### Requirement: Footer filters echo removed
The footer status line SHALL NOT display the `filters` field that echoes the resolved search query.

#### Scenario: Footer with active filters
- **WHEN** a search query with filter tokens (e.g., `agent:claude`) is active
- **THEN** the footer SHALL NOT show a `filters agent:claude` echo
- **THEN** the footer SHALL continue to show sync status, PR pending count, and warnings

### Requirement: Footer right-side status is lower priority than left-side hints
The footer's left-side key-hints SHALL be the higher-priority half and the right-side status (sync / PR-pending / warning) SHALL be the lower-priority half. When the footer row cannot show both halves in full — the left hints, a minimum gap, and the right status together exceeding the row width — the right-side status SHALL be hidden entirely and the full row given to the left-side hints. The right-side status SHALL NOT clip, overlap the hints, or displace the hints.

#### Scenario: Both halves fit
- **WHEN** the footer row is wide enough to show the left hints, a minimum gap, and the right status in full
- **THEN** the left hints SHALL render at the left and the right status SHALL render right-aligned at the right (current layout)

#### Scenario: Not enough space for both
- **WHEN** the footer row is not wide enough to show both halves with the minimum gap
- **THEN** the right-side status SHALL be hidden entirely
- **AND** the left-side hints SHALL be given the full footer row width

#### Scenario: Right status empty
- **WHEN** the right-side status has no content (no sync, no PR-pending, no warning)
- **THEN** the footer SHALL render only the left-side hints, occupying the full row
