# Quarantine Directory

This directory contains test files for secret scanning validation.

## Purpose

These files are used to test the secret scanning functionality in CI:

- **Positive Control**: Files that MUST cause the scanner to fail (contain fake secrets)
- **Negative Control**: Files that MUST pass the scanner (clean content)

## Files

- `positive-control-fake-secrets.txt` - Contains fake tokens that should trigger scanner
- `negative-control-clean.txt` - Contains clean content that should pass scanner

## Important

These files are for testing purposes only and contain FAKE credentials that are not valid.
